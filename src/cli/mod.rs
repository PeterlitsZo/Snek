use std::{ffi::OsStr, path::PathBuf};

use clap::{
    Parser,
    builder::{PathBufValueParser, TypedValueParser},
};

#[derive(Parser)]
#[command(
    name = "snek",
    version,
    override_usage = "snek [OPTIONS] <FILE> [ARGS]...",
    after_help = "Arguments after FILE are passed unchanged to the script.",
    about = "A collection of practical utilities with a TypeScript scripting runtime"
)]
pub(crate) struct Args {
    /// TypeScript file to execute.
    #[arg(value_name = "FILE", value_parser = PathBufValueParser::new().try_map(validate_script))]
    pub(crate) script: PathBuf,
    #[arg(skip)]
    pub(crate) argv: Vec<String>,
}

fn validate_script(path: PathBuf) -> Result<PathBuf, String> {
    if path.extension() != Some(OsStr::new("ts"))
        || path
            .file_name()
            .is_some_and(|name| name.as_encoded_bytes().ends_with(b".d.ts"))
    {
        return Err("expected a .ts script (declaration files are not executable)".into());
    }
    Ok(path)
}

impl Args {
    pub(crate) fn from_env() -> Self {
        // Only parse the host prefix with clap: everything after FILE belongs
        // to the script, including its first "--" and built-in flag names.
        let raw: Vec<_> = std::env::args_os().collect();
        let mut after_separator = false;
        let boundary = raw
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(index, value)| {
                if after_separator || !value.as_encoded_bytes().starts_with(b"-") || value == "-" {
                    return Some(index + 1);
                }
                after_separator = value == "--";
                None
            })
            .unwrap_or(raw.len());
        let mut args = Self::parse_from(&raw[..boundary]);
        args.argv = raw[boundary..]
            .iter()
            .map(|value| {
                value.clone().into_string().unwrap_or_else(|_| {
                    clap::Error::raw(
                        clap::error::ErrorKind::InvalidUtf8,
                        "script arguments must be valid UTF-8",
                    )
                    .exit()
                })
            })
            .collect();
        args
    }
}
