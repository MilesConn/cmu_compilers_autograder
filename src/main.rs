use clap::Parser;
use new_grader::{config::Cli, run, runner::make_and_run};

extern crate tempdir;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
