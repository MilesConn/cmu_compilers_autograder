use std::path::PathBuf;

use clap::Parser;

// TODO: get rid of unused options

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Terminal coloring
    #[arg(short = 'c', long, value_parser = ["on", "off"])]
    pub color: Option<String>,

    /// Quiet (use -q through -qqqqqqq)
    #[arg(short = 'q', action = clap::ArgAction::Count)]
    pub quiet: u8,

    /// Use symlinked files for tests
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Don't rebuild bin/c0c
    #[arg(long)]
    pub nomake: bool,

    /// Build compiler as 'make <lab>'
    #[arg(short = 'm', long)]
    pub make: Option<String>,

    /// Path to reference compiler
    #[arg(long)]
    pub cc0: Option<String>,

    /// Number of tests to run in parallel
    #[arg(short = 'j', long, value_parser = clap::value_parser!(u32))]
    pub parallel: Option<u32>,

    /// Add comma-separated args for compiler
    #[arg(short = 'a', long)]
    pub args: Option<String>,

    /// Compiler variant (x86-64, exe, llvm)
    #[arg(short = 'e', long, value_parser = ["x86-64", "exe", "llvm"], default_value = "x86-64")]
    pub emit: String,

    /// Whether to run only those tests given in keep.txt
    #[arg(long)]
    pub prune: bool,

    /// Whether to run verifier mac executable
    #[arg(long)]
    pub mac: bool,

    /// The directory containing binaries for performing static analysis
    #[arg(long)]
    pub static_analysis_dir: Option<String>,

    /// Whether to fail duplicate tests. (NOTE: requires static-analysis-dir)
    #[arg(long)]
    pub fail_duplicate_tests: bool,

    /// Whether to print a warning message on duplicate tests. (NOTE: requires static-analysis-dir)
    #[arg(long)]
    pub warn_duplicate_tests: bool,

    /// Whether to fail buggy tests. (NOTE: requires static-analysis-dir)
    #[arg(long)]
    pub fail_dodgy_tests: bool,

    /// Whether to run only unsafe (i.e., mem-error, div-by-zero) tests
    #[arg(long)]
    pub unsafe_only: bool,

    /// Whether to run only safe (i.e., returning, typecheck) tests
    #[arg(long)]
    pub safe_only: bool,

    /// Whether to only run each test for typechecking, not runtime.
    #[arg(long)]
    pub typecheck_only: bool,

    /// If present, allow infloop tests.
    #[arg(long)]
    pub allow_infloop_tests: bool,

    /// Compiler build time limit (1800 seconds)
    #[arg(long, value_parser = clap::value_parser!(u32), default_value = "1800")]
    pub limit_make: u32,

    /// Typechecker time limit (4 seconds)
    #[arg(long, value_parser = clap::value_parser!(u32), default_value = "4")]
    pub limit_tc: u32,

    /// Compiler time limit (6 seconds)
    #[arg(long, value_parser = clap::value_parser!(u32), default_value = "6")]
    pub limit_compile: u32,

    /// Linker time limit (8 seconds)
    #[arg(long, value_parser = clap::value_parser!(u32), default_value = "8")]
    pub limit_link: u32,

    /// Execution time limit (5 seconds)
    #[arg(long, value_parser = clap::value_parser!(u32), default_value = "5")]
    pub limit_run: u32,

    /// Max length of a filename (37 chars)
    #[arg(long, value_parser = clap::value_parser!(u32), default_value = "37")]
    pub limit_filename: u32,

    /// Relaxed test case validation
    #[arg(long)]
    pub relax: bool,

    /// Delete all log files
    #[arg(long)]
    pub nolog: bool,

    /// Debug information
    #[arg(long)]
    pub debug: bool,

    /// Only test specific extension
    #[arg(short = 'f', long)]
    pub filter: Option<String>,

    /// Run l2 checkpoint for forward-may direction
    #[arg(long)]
    pub forward_may: bool,

    /// Run l2 checkpoint for forward-must direction
    #[arg(long)]
    pub forward_must: bool,

    /// Run l2 checkpoint for backward-must direction
    #[arg(long)]
    pub backward_must: bool,

    /// Run l2 checkpoint for backward-may direction
    #[arg(long)]
    pub backward_may: bool,

    /// Produce autograder output
    #[arg(long)]
    pub autograder: bool,

    /// Path to test directory
    pub path: PathBuf,
}
