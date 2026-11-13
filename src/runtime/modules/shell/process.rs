use std::process::Stdio;

use boa_engine::{JsResult, JsValue};

use super::shell_error;

pub(super) async fn run_shell(words: Vec<String>) -> JsResult<JsValue> {
    let program = &words[0];
    let mut child = tokio::process::Command::new(program)
        .args(&words[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| shell_error(format!("could not start '{program}': {error}")))?;
    let status = child
        .wait()
        .await
        .map_err(|error| shell_error(format!("could not wait for '{program}': {error}")))?;
    if !status.success() {
        return Err(shell_error(format!("'{program}' failed: {status}")));
    }
    Ok(JsValue::undefined())
}
