use std::path::Path;

use anyhow::{Result, anyhow};
use boa_engine::{Context, Source};
use boa_runtime::{Console, DefaultLogger};

pub(crate) fn execute(path: &Path, javascript: &str) -> Result<()> {
    let mut context = Context::default();
    Console::register_with_logger(DefaultLogger, &mut context)
        .map_err(|error| anyhow!("console initialization failed: {error}"))?;

    // Deliberately evaluate as a script and leave the Promise job queue undriven.
    // Keep the filename for Boa diagnostics; positions refer to generated JavaScript.
    context
        .eval(Source::from_bytes(javascript).with_path(path))
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}
