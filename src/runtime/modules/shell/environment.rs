use std::{collections::BTreeMap, path::Path};

use crate::runtime::state::RuntimeState;

use tokio::process::Command;

/// Shared script directory and independently cloned command environment overrides.
#[derive(Clone)]
pub(in crate::runtime) struct ShellEnvironment {
    state: RuntimeState,
    pub(super) variables: BTreeMap<String, String>,
}

pub(super) enum PreparedCommand {
    External(Command),
    Builtin(u8),
}

impl ShellEnvironment {
    pub(in crate::runtime) fn new(state: RuntimeState) -> Self {
        Self {
            state,
            variables: BTreeMap::new(),
        }
    }

    /// Execute a builtin or prepare a child with a snapshot of the current directory.
    pub(super) fn prepare(&self, words: &[String]) -> Result<PreparedCommand, String> {
        let mut state = self
            .state
            .0
            .lock()
            .map_err(|_| "shell directory lock poisoned")?;
        let program = &words[0];
        if program == "cd" {
            if words.len() != 2 || words[1].is_empty() || words[1].starts_with('-') {
                return Err("usage: cd PATH (exactly one path, no options)".into());
            }
            state
                .chdir(Path::new(&words[1]))
                .map_err(|error| format!("could not cd to '{}': {error}", words[1]))?;
            return Ok(PreparedCommand::Builtin(0));
        }

        if program == "exit" {
            let code = match &words[1..] {
                [] => 0,
                [value] if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
                    value
                        .parse::<u8>()
                        .map_err(|_| "usage: exit [CODE] (integer from 0 to 255)")?
                }
                _ => return Err("usage: exit [CODE] (integer from 0 to 255)".into()),
            };
            return Ok(PreparedCommand::Builtin(code));
        }

        // Resolve explicit relative executable paths ourselves: platform behavior
        // when combining such paths with current_dir() otherwise varies.
        let path = Path::new(program);
        let executable =
            if path.is_relative() && path.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
                state.cwd.join(path)
            } else {
                path.to_path_buf()
            };
        let mut command = Command::new(executable);
        command
            .args(&words[1..])
            .current_dir(&state.cwd)
            .env_clear()
            .envs(&state.variables)
            .envs(&self.variables);
        Ok(PreparedCommand::External(command))
    }
}
