use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

pub const ANSWERING_ELSEWHERE: &str =
    "This conversation is answering in another window. Wait for it to finish, then send again.";

const TRIES: usize = 16;

#[derive(Debug)]
struct Held {
    file: File,
    path: PathBuf,
}

impl Drop for Held {
    fn drop(&mut self) {
        if platform::LOCK_FILES_CAN_BE_REMOVED && platform::is_file_at(&self.file, &self.path) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Default)]
pub struct Hold(Option<Held>);

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
    for _ in 0..TRIES {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        match file.try_lock() {
            Ok(()) if platform::is_file_at(&file, path) => {
                return Ok(Hold(Some(Held { file, path: path.to_owned() })));
            }
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(ANSWERING_ELSEWHERE.to_owned()),
            Err(TryLockError::Error(error)) => return Err(error.to_string()),
        }
    }
    Err(ANSWERING_ELSEWHERE.to_owned())
}

#[cfg(test)]
#[path = "hold_tests.rs"]
mod tests;
