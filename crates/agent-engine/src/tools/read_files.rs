use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Unread,
    Changed,
    Partial,
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy)]
struct Read {
    stamp: Option<Stamp>,
    whole: bool,
}

#[derive(Debug, Default)]
pub struct ReadFiles(Mutex<HashMap<PathBuf, Read>>);

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(Stamp { len: metadata.len(), modified: metadata.modified().ok() })
}

impl ReadFiles {
    pub fn mark(&self, path: &Path, whole: bool) {
        let Ok(mut read) = self.0.lock() else { return };
        let whole = whole || read.get(path).is_some_and(|before| before.whole && before.stamp == stamp(path));
        read.insert(path.to_path_buf(), Read { stamp: stamp(path), whole });
    }

    pub fn freshness(&self, path: &Path) -> Freshness {
        let Ok(read) = self.0.lock() else { return Freshness::Unread };
        match read.get(path) {
            None => Freshness::Unread,
            Some(before) if before.stamp != stamp(path) => Freshness::Changed,
            Some(before) if !before.whole => Freshness::Partial,
            Some(_) => Freshness::Current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_changed_on_disk_after_it_was_read_is_stale() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one").expect("write");
        let files = ReadFiles::default();
        assert_eq!(files.freshness(&path), Freshness::Unread);
        files.mark(&path, true);
        assert_eq!(files.freshness(&path), Freshness::Current);
        std::fs::write(&path, "changed by a formatter").expect("write");
        assert_eq!(files.freshness(&path), Freshness::Changed);
        files.mark(&path, true);
        assert_eq!(files.freshness(&path), Freshness::Current, "reading again catches up");
    }

    #[test]
    fn a_part_read_stays_partial_until_the_whole_file_is_read() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one\ntwo").expect("write");
        let files = ReadFiles::default();
        files.mark(&path, false);
        assert_eq!(files.freshness(&path), Freshness::Partial);
        files.mark(&path, true);
        files.mark(&path, false);
        assert_eq!(files.freshness(&path), Freshness::Current, "a later slice does not forget the whole read");
    }
}
