use std::{ffi::OsStr, path::PathBuf};

use clap::{
    Parser,
    builder::{PathBufValueParser, TypedValueParser},
};

#[derive(Parser)]
#[command(
    name = "snek",
    version,
    about = "A collection of practical utilities with a TypeScript scripting runtime"
)]
pub(crate) struct Args {
    /// TypeScript file to execute (synchronous, no modules)
    #[arg(value_name = "FILE", value_parser = PathBufValueParser::new().try_map(validate_script))]
    pub(crate) script: PathBuf,
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
