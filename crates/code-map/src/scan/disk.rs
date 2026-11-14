use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ignore::WalkBuilder;

use super::{FileStat, Source};
use crate::extract::supported;

pub const MAX_FILE_BYTES: u64 = 512 * 1024;
const SKIPPED_DIRS: [&str; 2] = ["target", "node_modules"];

#[derive(Debug, Clone)]
pub struct DiskSource {
    root: PathBuf,
}

impl DiskSource {
    pub fn new(root: &Path) -> Self {
        DiskSource { root: root.to_path_buf() }
    }
}

impl Source for DiskSource {
    fn files(&self) -> Vec<FileStat> {
        let walker = WalkBuilder::new(&self.root)
            .require_git(false)
            .filter_entry(|entry| {
                !(entry.file_type().is_some_and(|kind| kind.is_dir())
                    && SKIPPED_DIRS.iter().any(|skipped| entry.file_name() == *skipped))
            })
            .build();
        let mut files: Vec<FileStat> =
            walker.filter_map(Result::ok).filter_map(|entry| self.stat(entry.path())).collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }

    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(path)).ok()
    }
}

impl DiskSource {
    fn stat(&self, path: &Path) -> Option<FileStat> {
        let relative = path.strip_prefix(&self.root).ok()?;
        let relative: Vec<String> =
            relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
        let relative = relative.join("/");
        if !supported(&relative) {
            return None;
        }
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return None;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .and_then(|elapsed| i64::try_from(elapsed.as_nanos()).ok())
            .unwrap_or(0);
        Some(FileStat { path: relative, len: metadata.len(), modified })
    }
}
