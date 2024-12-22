use runner::make_and_run;

pub mod config;
mod pipeline;
pub mod runner;
pub mod runner_file_utils;
pub mod test_parser;

pub fn run(cli: config::Cli) -> anyhow::Result<()> {
    make_and_run(cli.path.clone(), cli)?;

    Ok(())
}
