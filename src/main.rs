mod cli;
mod runtime;
mod transpile;

use std::{fs, process::ExitCode};

use anyhow::{Context, Result};
use clap::Parser;

fn main() -> ExitCode {
    let args = cli::Args::parse();
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: cli::Args) -> Result<()> {
    let path = &args.script;
    let source = fs::read_to_string(path)
        .with_context(|| format!("read failed for '{}'", path.display()))?;
    let javascript = transpile::transpile(path, &source)
        .with_context(|| format!("transpile failed for '{}'", path.display()))?;
    runtime::execute(path, &javascript)
        .with_context(|| format!("execution failed for '{}'", path.display()))
}
