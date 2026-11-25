mod cli;
mod runtime;
mod transpile;

use std::{fs, process::ExitCode};

use anyhow::{Context, Result};

fn main() -> ExitCode {
    let args = cli::Args::from_env();
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
    let executable = std::env::current_exe().context("resolve executable path")?;
    let script = fs::canonicalize(path).context("resolve script path")?;
    let mut argv = vec![
        executable.to_string_lossy().into_owned(),
        script.to_string_lossy().into_owned(),
    ];
    argv.extend(args.argv);
    runtime::execute(path, &script, &javascript, argv)
        .with_context(|| format!("execution failed for '{}'", path.display()))
}
