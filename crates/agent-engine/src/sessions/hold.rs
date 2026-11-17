use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

pub const ANSWERING_ELSEWHERE: &str =
    "This conversation is answering in another window. Wait for it to finish, then send again.";

#[derive(Debug, Default)]
pub struct Hold(Option<File>);

impl Hold {
    pub fn none() -> Self {
        Self(None)
    }

    pub fn held(&self) -> bool {
        self.0.is_some()
    }
}

pub(super) fn take(path: &Path) -> Result<Hold, String> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    }
    let file =
        OpenOptions::new().create(true).truncate(false).write(true).open(path).map_err(|error| error.to_string())?;
    match file.try_lock() {
        Ok(()) => Ok(Hold(Some(file))),
        Err(TryLockError::WouldBlock) => Err(ANSWERING_ELSEWHERE.to_owned()),
        Err(TryLockError::Error(error)) => Err(error.to_string()),
    }
}

#[cfg(test)]
#[path = "hold_tests.rs"]
mod tests;
