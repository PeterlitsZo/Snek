use std::process::Stdio;

use boa_engine::JsResult;

use super::{ShellEnvironment, shell_error};

/// Output routing, fixed when the command starts. Stdin stays inherited.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputMode {
    /// Write directly to the parent's stdout without saving output.
    Terminal,
    /// Collect stdout bytes for repeated reads through text(), json(), etc.
    Capture,
    /// Collect both output streams for nothrow().
    FullCapture,
}

pub(super) struct ProcessOutput {
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) exit_code: Option<i32>,
    pub(super) failure: Option<String>,
}

pub(super) async fn run_shell(
    words: Vec<String>,
    mode: OutputMode,
    environment: ShellEnvironment,
) -> JsResult<ProcessOutput> {
    let Some(mut command) = environment.prepare(&words).map_err(shell_error)? else {
        return Ok(ProcessOutput {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: Some(0),
            failure: None,
        });
    };
    let program = &words[0];
    let mut child = command
        .stdin(Stdio::inherit())
        .stdout(match mode {
            OutputMode::Terminal => Stdio::inherit(),
            OutputMode::Capture | OutputMode::FullCapture => Stdio::piped(),
        })
        .stderr(if mode == OutputMode::FullCapture {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| shell_error(format!("could not start '{program}': {error}")))?;
    let (status, stdout, stderr) = match mode {
        OutputMode::Terminal => (
            child
                .wait()
                .await
                .map_err(|error| shell_error(format!("could not wait for '{program}': {error}")))?,
            Vec::new(),
            Vec::new(),
        ),
        OutputMode::Capture | OutputMode::FullCapture => {
            // Tokio drains both pipes while waiting, including output larger than pipe capacity.
            let output = child.wait_with_output().await.map_err(|error| {
                shell_error(format!(
                    "could not capture output from '{program}': {error}"
                ))
            })?;
            (output.status, output.stdout, output.stderr)
        }
    };
    Ok(ProcessOutput {
        stdout,
        stderr,
        exit_code: status.code(),
        failure: (!status.success()).then(|| format!("'{program}' failed: {status}")),
    })
}
