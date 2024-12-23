use anyhow::{anyhow, bail, Context, Error, Result};
use colored::Colorize;
use std::os::unix::process::ExitStatusExt;
use std::process::Stdio;
use std::{
    env, fs,
    path::{self, Path, PathBuf},
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;
use wait_timeout::ChildExt;

use std::fs::File;
use std::io::{self, BufRead, Read, Write};
use tempdir::TempDir;

use crate::{
    config::Cli,
    runner_file_utils::process_files_parallel,
    test_parser::{self, TestResult},
};

#[derive(Debug)]
enum TestOutcome {
    Passed,   // 1.0
    TimedOut, // -0.1
    Failed,   // -1.0
              // TODO: store incorrect result
}

#[derive(Error, Debug)]
enum TestFailure {
    #[error("Compiler failed")]
    CompileFailure, // -1.0
    #[error("Test was malformed")]
    MalformedTest, // 0.0
}

#[derive(Debug)]
pub enum ProcessResult {
    Success(i32),
    Failure(i32),
    Timeout,
    SignalAbort,
    SignalUsr2,
    SigFpe,
    OtherSignal(i32),
}

fn add_extension(path: &PathBuf, extension: impl AsRef<Path>) -> PathBuf {
    let mut path = path.clone();
    match path.extension() {
        Some(ext) => {
            let mut ext = ext.to_os_string();
            ext.push(".");
            ext.push(extension.as_ref());
            path.set_extension(ext);
            path
        }
        None => {
            path.set_extension(extension.as_ref());
            path
        }
    }
}

pub fn make_and_run<P>(path: P, config: Cli) -> Result<f32>
where
    P: AsRef<Path>,
{
    rayon::ThreadPoolBuilder::new()
        .num_threads(config.parallel.unwrap_or(1).try_into().unwrap())
        .build_global()
        .unwrap();
    // Assume Make is in CWD
    {
        let mut make_cmd = Command::new("make");
        if let Some(par) = config.parallel {
            make_cmd.arg(format!("-j {par}"));
        }

        let status = make_cmd.status()?;
        if !status.success() {
            bail!("Expected make to succeed but failed");
        }
    }

    // Student compiler should be made and now exists in
    // CWD/bin
    //
    let student_compiler_path = path::absolute(Path::new("./bin/c0c"))?;
    // let runtime_path = path::absolute(Path::new("../runtime"))?;

    if !student_compiler_path.exists() {
        bail!("Expected ./bin/c0c to exist");
    }

    // This is the main business logic
    let run_and_verify = |p: &PathBuf| -> Result<TestOutcome> {
        let intended_result = test_parser::get_test_result(p)
            .with_context(|| format!("Test {p:?} failed to parse"))?;

        let tempdir = TempDir::new("c0_runner").unwrap();

        let runtime_path = path::absolute(Path::new("../runtime"))?;
        // let absolute_test_path = path::absolute(p).unwrap();

        // TODO: this is a race condition lol ...
        // env::set_current_dir(&tempdir).unwrap();
        //
        let test_name = p
            .file_name()
            .ok_or(anyhow!("Couldn't extract file name from p"))?;
        let new_test_path = tempdir.path().join(test_name);
        fs::copy(p, &new_test_path)?;
        // Symlinks might be weird...
        // symlink(p, &new_test_path)?;
        //
        let stdout_pipe = Stdio::piped();

        // TODO: add user supported args
        let compiler_output = Command::new(student_compiler_path.clone())
            .arg("-ex86-64")
            .arg(new_test_path.to_str().unwrap())
            .output()
            .with_context(|| "Student compiler failed")?;

        if matches!(intended_result, TestResult::SourceError) {
            return if !compiler_output.status.success() {
                Ok(TestOutcome::Passed)
            } else {
                Err(TestFailure::CompileFailure)
                    .with_context(|| String::from_utf8_lossy(&compiler_output.stdout).to_string())
            };
        }

        if !compiler_output.status.success() {
            bail!("Student compiler failed");
        }

        let paths = fs::read_dir(&tempdir).unwrap();

        let out_path = tempdir.path().join("a.out");

        // We should now have a a.out output file
        // TODO: handle linking
        let linked_output = Command::new("gcc")
            .args([
                "-g",
                "-fno-stack-protector",
                "-fno-lto",
                "-fno-asynchronous-unwind-tables",
                #[cfg(target_os = "macos")]
                "-target",
                #[cfg(target_os = "macos")]
                "x86_64-apple-darwin", // TODO:
                "-O0",
                "-o",
                out_path.to_str().unwrap(),
                add_extension(&new_test_path, "s").to_str().unwrap(),
                runtime_path.join("run411.c").to_str().unwrap(),
            ])
            .output()
            .with_context(|| "GCC failed to link")?;

        if !linked_output.status.success() {
            bail!(
                "Failed to link with: \n\t{}",
                String::from_utf8_lossy(&compiler_output.stdout).to_string()
            );
        }

        // // spawn compiled process
        // let mut child = command::new(out_path).output().unwrap();;

        let mut child = Command::new(out_path).stdout(Stdio::piped()).spawn()?;
        let run_timeout = Duration::from_secs(config.limit_run as u64);
        let status_code = child.wait_timeout(run_timeout)?;

        let execution_result: ProcessResult = match status_code {
            Some(status) => {
                if status.success() {
                    let child_stdout = child.stdout.take().unwrap();
                    let last_line =
                        String::from_utf8(child_stdout.bytes().collect::<Result<Vec<_>, _>>()?)?
                            .lines()
                            .last()
                            .ok_or(anyhow!("No output"))?
                            .parse::<i32>()?;
                    ProcessResult::Success(last_line)
                } else {
                    if let Some(exit_code) = status.code() {
                        ProcessResult::Failure(exit_code)
                    } else {
                        match status.signal().unwrap() {
                            libc::SIGABRT => ProcessResult::SignalAbort,
                            libc::SIGFPE => ProcessResult::SigFpe,
                            libc::SIGUSR2 => ProcessResult::SignalUsr2,
                            other => ProcessResult::OtherSignal(other),
                        }
                    }
                }
            }
            None => {
                child.kill()?;
                child.wait()?;
                ProcessResult::Timeout
            }
        };

        Ok(match (intended_result, execution_result) {
            (TestResult::Ret(r), ProcessResult::Success(o)) => {
                if r == o {
                    println!("{}", format!("Test {test_name:?} passed").green());
                    TestOutcome::Passed
                } else {
                    println!(
                        "{}",
                        format!("{test_name:?} failed: expected {r} got {o}.").red()
                    );
                    TestOutcome::Failed
                }
            }
            (TestResult::Abort, ProcessResult::SignalAbort)
            | (TestResult::MemError, ProcessResult::SignalUsr2)
            | (TestResult::DivByZero, ProcessResult::SigFpe) => {
                println!("{}", format!("Test {test_name:?} passed").green());
                TestOutcome::Passed
            }
            (_, ProcessResult::Timeout) => {
                println!("{}", format!("{test_name:?} timed out").yellow());
                TestOutcome::TimedOut
            }
            // TODO: handle this case with logging
            _ => TestOutcome::Failed,
        })
    };

    let map_score = |p: &PathBuf, r: Result<TestOutcome>| -> f32 {
        let test_name = p.file_name().unwrap();
        match r {
            Ok(TestOutcome::Passed) => {
                println!("{}", format!("Test {test_name:?} passed").green());
                1.0
            }
            Ok(TestOutcome::TimedOut) => -0.1,
            Ok(TestOutcome::Failed) => -1.0,
            Err(e) => {
                println!(
                    "{}",
                    format!("{test_name:?} failed with error\n\t {e}").red()
                );
                -1.0
            }
        }
    };

    let score = process_files_parallel(path, |p: &PathBuf| map_score(p, run_and_verify(p)));

    score.map(|v| v.iter().fold(0.0, |acc, e| acc + e))
}
