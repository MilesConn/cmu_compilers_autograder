use runner::make_and_run;

pub mod config;
mod pipeline;
pub mod runner;
pub mod runner_file_utils;
pub mod test_parser;

pub fn run(cli: config::Cli) -> anyhow::Result<()> {
    let s = make_and_run(cli.path.clone(), &cli)?;

    println!("Score: {}", s.to_score());

    if cli.autograder {
        println!("{}", serde_json::to_string(&s).unwrap());
    }

    Ok(())
}
