use std::{
    collections::BTreeMap,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Script-local state shared by process APIs and child commands.
#[derive(Clone)]
pub(super) struct RuntimeState(pub(super) Arc<Mutex<ProcessState>>);

pub(super) struct ProcessState {
    pub(super) cwd: PathBuf,
    pub(super) variables: BTreeMap<OsString, OsString>,
    pub(super) exit_code: u8,
}

impl RuntimeState {
    pub(super) fn new() -> io::Result<Self> {
        Ok(Self(Arc::new(Mutex::new(ProcessState {
            cwd: std::env::current_dir()?,
            variables: std::env::vars_os().collect(),
            exit_code: 0,
        }))))
    }
}

impl ProcessState {
    pub(super) fn chdir(&mut self, path: &Path) -> io::Result<()> {
        let resolved = self.cwd.join(path).canonicalize()?;
        // Check search permission on the final directory as well.
        if !resolved.join(".").metadata()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "not a directory",
            ));
        }
        self.cwd = resolved;
        Ok(())
    }
}
