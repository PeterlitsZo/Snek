use std::process::Stdio;

use boa_engine::JsResult;

use super::{ShellEnvironment, shell_error};

/// Where stdout goes; fixed when the command starts. Stdin and stderr stay inherited.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputMode {
    /// Write directly to the parent's stdout without saving output.
    Terminal,
    /// Collect stdout bytes for repeated reads through text(), json(), etc.
    Capture,
}

pub(super) async fn run_shell(
    words: Vec<String>,
    mode: OutputMode,
    environment: ShellEnvironment,
) -> JsResult<Vec<u8>> {
    let Some(mut command) = environment.prepare(&words).map_err(shell_error)? else {
        return Ok(Vec::new());
    };
    let program = &words[0];
    let mut child = command
        .stdin(Stdio::inherit())
        .stdout(match mode {
            OutputMode::Terminal => Stdio::inherit(),
            OutputMode::Capture => Stdio::piped(),
        })
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| shell_error(format!("could not start '{program}': {error}")))?;
    let (status, bytes) = match mode {
        OutputMode::Terminal => (
            child
                .wait()
                .await
                .map_err(|error| shell_error(format!("could not wait for '{program}': {error}")))?,
            Vec::new(),
        ),
        OutputMode::Capture => {
            // Tokio drains the pipe while waiting, including output larger than pipe capacity.
            let output = child.wait_with_output().await.map_err(|error| {
                shell_error(format!(
                    "could not capture output from '{program}': {error}"
                ))
            })?;
            (output.status, output.stdout)
        }
    };
    if !status.success() {
        return Err(shell_error(format!("'{program}' failed: {status}")));
    }
    Ok(bytes)
}
