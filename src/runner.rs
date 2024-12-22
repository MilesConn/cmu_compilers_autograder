use anyhow::{bail, Context, Error, Result};
use std::{
    env,
    path::{self, Path, PathBuf},
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

use std::fs::File;
use std::io::{self, Write};
use tempdir::TempDir;

use crate::{
    config::Cli,
    runner_file_utils::process_files_parallel,
    test_parser::{self, TestResult},
};

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
    Timeout,
    SignalAbort,
    SignalUsr2,
    SigFpe,
    OtherSignal(i32),
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
        let intended_result =
            test_parser::get_test_result(p).map_err(|_| TestFailure::MalformedTest)?;

        let tempdir = TempDir::new("c0_runner").unwrap();

        let runtime_path = path::absolute(Path::new("../runtime"))?;
        let absolute_test_path = path::absolute(p).unwrap();

        env::set_current_dir(&tempdir).unwrap();

        // TODO: add user supported args
        let compiler_exit_status = Command::new(student_compiler_path.clone())
            .arg(absolute_test_path.to_str().unwrap())
            .status()
            .map_err(|_| TestFailure::CompileFailure)?;

        if matches!(intended_result, TestResult::SourceError) {
            return match compiler_exit_status.code() {
                Some(1) => Ok(TestOutcome::Passed),
                _ => Ok(TestOutcome::Failed),
            };
        }

        // We should now have a a.out output file
        // TODO: handle linking
        let linked_status = Command::new("gcc")
            .args([
                "-g",
                "-fno-stack-protector",
                "-fno-lto",
                "-fno-asynchronous-unwind-tables",
                "-O0",
                "./a.out",
                runtime_path.join("run411.c").to_str().unwrap(),
            ])
            .status()
            .map_err(|_| TestFailure::CompileFailure)?;

        if !linked_status.success() {
            bail!("Failed to link");
        }

        let start_time = Instant::now();

        // Spawn compiled process
        let mut child = Command::new("./a.out").spawn().unwrap();

        let execution_result: ProcessResult = loop {
            // Check if process has completed
            match child.try_wait() {
                Ok(Some(status)) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;

                        // Check if process was terminated by a signal
                        if let Some(signal) = status.signal() {
                            break Ok(match signal {
                                libc::SIGABRT => ProcessResult::SignalAbort,
                                libc::SIGFPE => ProcessResult::SigFpe,
                                libc::SIGUSR2 => ProcessResult::SignalUsr2,
                                other => ProcessResult::OtherSignal(other),
                            });
                        }
                    }

                    // Check exit code
                    break Ok(match status.code() {
                        Some(code) => ProcessResult::Success(code),
                        None => ProcessResult::OtherSignal(0), // Process terminated by an unknown signal
                    });
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

        return Ok(match (intended_result, execution_result) {
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
        });
    };

    let map_score = |r: Result<TestOutcome>| -> f32 {
        match r {
            Ok(TestOutcome::Passed) => 1.0,
            Ok(TestOutcome::TimedOut) => -0.1,
            Ok(TestOutcome::Failed) | Err(_) => -1.0,
        }
    };

    let score = process_files_parallel(path, |p: &PathBuf| map_score(run_and_verify(p)));

    score.map(|v| v.iter().fold(0.0, |acc, e| acc + e))
}
