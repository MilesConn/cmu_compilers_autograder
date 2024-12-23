use anyhow::{anyhow, bail, Context, Error, Result};
use clap::builder::OsStr;
use std::os::unix::fs::symlink;
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
        println!("Running on {p:?}");
        let intended_result = test_parser::get_test_result(p)
            .with_context(|| format!("Test {p:?} failed to parse"))?;

        let tempdir = TempDir::new("c0_runner").unwrap();

        let runtime_path = path::absolute(Path::new("../runtime"))?;
        println!("RUNTIME PATH {runtime_path:?}");
        // let absolute_test_path = path::absolute(p).unwrap();

        // TODO: this is a race condition lol ...
        // env::set_current_dir(&tempdir).unwrap();
        //
        let test_name = p
            .file_name()
            .ok_or(anyhow!("Couldn't extract file name from p"))?;
        let new_test_path = tempdir.path().join(test_name);
        println!("NEW TEST PATH {:?}", new_test_path);
        println!("P: {:?}", p);
        println!("P EXISTS? {}", p.exists());
        fs::copy(p, &new_test_path)?;
        // Symlinks might be weird...
        // symlink(p, &new_test_path)?;

        // TODO: add user supported args
        let compiler_exit_status = Command::new(student_compiler_path.clone())
            .arg("-ex86-64")
            .arg(new_test_path.to_str().unwrap())
            .status()
            .with_context(|| "Student compiler failed")?;

        if matches!(intended_result, TestResult::SourceError) {
            return match compiler_exit_status.code() {
                Some(1) => Ok(TestOutcome::Passed),
                _ => Ok(TestOutcome::Failed),
            };
        }

        if !compiler_exit_status.success() {
            bail!("Student compiler failed");
        }

        println!("TEMP DIR {:?}", tempdir);
        let paths = fs::read_dir(&tempdir).unwrap();

        for path in paths {
            println!("Name: {}", path.unwrap().path().display())
        }
        println!("Done listing fiels");

        let out_path = tempdir.path().join("a.out");

        // We should now have a a.out output file
        // TODO: handle linking
        let linked_status = Command::new("gcc")
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
            .status()
            .with_context(|| "GCC failed to link")?;

        if !linked_status.success() {
            bail!("Failed to link");
        }

        let start_time = Instant::now();

        // // spawn compiled process
        // let mut child = command::new(out_path).output().unwrap();;

        // TODO:
        // Need coreutilsld run our own timer ...
        let mut child = Command::new(out_path).stdout(Stdio::piped()).spawn()?;

        // Instead of running a loop here... we could just spawn it with a timer
        let execution_result: ProcessResult = loop {
            // Check if process has completed
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        let child_stdout = child.stdout.take().unwrap();
                        let last_line =
                            String::from_utf8(
                                child_stdout.bytes().collect::<Result<Vec<_>, _>>()?,
                            )?
                            .lines()
                            .last()
                            .ok_or(anyhow!("No output"))?
                            .parse::<i32>()?;
                        break Ok(ProcessResult::Success(last_line));
                    } else {
                        if let Some(exit_code) = status.code() {
                            break Ok(ProcessResult::Failure(exit_code));
                        } else {
                            break Ok(match status.signal().unwrap() {
                                libc::SIGABRT => ProcessResult::SignalAbort,
                                libc::SIGFPE => ProcessResult::SigFpe,
                                libc::SIGUSR2 => ProcessResult::SignalUsr2,
                                other => ProcessResult::OtherSignal(other),
                            });
                        }
                    };
                }
                Ok(None) => {
                    // Process still running, check timeout
                    if start_time.elapsed() >= Duration::from_secs(config.limit_run as u64) {
                        // Kill the process
                        let _ = child.kill();
                        let _ = child.wait();
                        break Ok(ProcessResult::Timeout);
                    }
                    // Sleep briefly to prevent busy waiting
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    // Try to clean up
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(e).context("Error while waiting for process");
                }
            }
        }?;
        println!("Intended result {intended_result:?}");
        println!("Process REsult {execution_result:?}");

        Ok(match (intended_result, execution_result) {
            (TestResult::Ret(r), ProcessResult::Success(o)) => {
                if r == o {
                    TestOutcome::Passed
                } else {
                    TestOutcome::Failed
                }
            }
            (TestResult::Abort, ProcessResult::SignalAbort)
            | (TestResult::MemError, ProcessResult::SignalUsr2)
            | (TestResult::DivByZero, ProcessResult::SigFpe) => TestOutcome::Passed,
            (_, ProcessResult::Timeout) => TestOutcome::TimedOut,
            _ => TestOutcome::Failed,
        })
    };

    let map_score = |r: Result<TestOutcome>| -> f32 {
        println!("RESULT {r:?}");
        match r {
            Ok(TestOutcome::Passed) => 1.0,
            Ok(TestOutcome::TimedOut) => -0.1,
            Ok(TestOutcome::Failed) | Err(_) => -1.0,
        }
    };

    let score = process_files_parallel(path, |p: &PathBuf| map_score(run_and_verify(p)));

    score.map(|v| v.iter().fold(0.0, |acc, e| acc + e))
}
