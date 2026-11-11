use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    len: u64,
    modified: Option<SystemTime>,
    offset: usize,
    limit: usize,
}

#[derive(Debug, Default)]
pub struct SeenFiles(Mutex<HashMap<PathBuf, Seen>>);

impl SeenFiles {
    pub fn seen_again(&self, path: &Path, offset: usize, limit: usize) -> bool {
        let Ok(metadata) = std::fs::metadata(path) else { return false };
        let now = Seen { len: metadata.len(), modified: metadata.modified().ok(), offset, limit };
        let Ok(mut seen) = self.0.lock() else { return false };
        seen.insert(path.to_path_buf(), now) == Some(now)
    }

    pub fn forget(&self) {
        if let Ok(mut seen) = self.0.lock() {
            seen.clear();
        }
    }
}
