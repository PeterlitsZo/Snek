use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use tokio::process::Command;

/// Shared script directory and independently cloned command environment overrides.
#[derive(Clone)]
pub(in crate::runtime) struct ShellEnvironment {
    cwd: Arc<Mutex<PathBuf>>,
    pub(super) variables: BTreeMap<String, String>,
}

impl ShellEnvironment {
    pub(in crate::runtime) fn new() -> io::Result<Self> {
        Ok(Self {
            cwd: Arc::new(Mutex::new(std::env::current_dir()?)),
            variables: BTreeMap::new(),
        })
    }

    /// Execute a builtin or prepare a child with a snapshot of the current directory.
    pub(super) fn prepare(&self, words: &[String]) -> Result<Option<Command>, String> {
        let mut cwd = self
            .cwd
            .lock()
            .map_err(|_| "shell directory lock poisoned")?;
        let program = &words[0];
        if program == "cd" {
            if words.len() != 2 || words[1].is_empty() || words[1].starts_with('-') {
                return Err("usage: cd PATH (exactly one path, no options)".into());
            }
            let target = cwd.join(&words[1]);
            let directory = (|| -> io::Result<PathBuf> {
                let resolved = target.canonicalize()?;
                // Resolving '.' also checks search permission on the final directory
                // on Unix, without requiring permission to list its contents.
                if !resolved.join(".").metadata()?.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        "not a directory",
                    ));
                }
                Ok(resolved)
            })()
            .map_err(|error| format!("could not cd to '{}': {error}", words[1]))?;
            *cwd = directory;
            return Ok(None);
        }

        // Resolve explicit relative executable paths ourselves: platform behavior
        // when combining such paths with current_dir() otherwise varies.
        let path = Path::new(program);
        let executable =
            if path.is_relative() && path.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
                cwd.join(path)
            } else {
                path.to_path_buf()
            };
        let mut command = Command::new(executable);
        command
            .args(&words[1..])
            .current_dir(&*cwd)
            .envs(&self.variables);
        Ok(Some(command))
    }
}
