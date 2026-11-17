use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

type Stamp = (u64, Option<SystemTime>);

#[derive(Debug)]
struct Seen {
    stamp: Stamp,
    ranges: Vec<(usize, usize)>,
}

#[derive(Debug, Default)]
pub struct SeenFiles {
    seen: Mutex<HashMap<PathBuf, Seen>>,
    unwatched: AtomicUsize,
}

#[derive(Debug)]
pub struct Unwatched<'a>(&'a SeenFiles);

impl Drop for Unwatched<'_> {
    fn drop(&mut self) {
        self.0.unwatched.fetch_sub(1, Ordering::SeqCst);
    }
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

impl SeenFiles {
    pub fn covers(&self, path: &Path, first: usize, last: usize) -> bool {
        if self.is_unwatched() {
            return false;
        }
        let (Some(now), Ok(seen)) = (stamp(path), self.seen.lock()) else { return false };
        first <= last
            && seen.get(path).is_some_and(|seen| {
                seen.stamp == now && seen.ranges.iter().any(|(from, to)| *from <= first && last <= *to)
            })
    }

    pub fn saw(&self, path: &Path, first: usize, last: usize) {
        if self.is_unwatched() {
            return;
        }
        let (Some(now), Ok(mut seen)) = (stamp(path), self.seen.lock()) else { return };
        let entry = seen.entry(path.to_path_buf()).or_insert_with(|| Seen { stamp: now, ranges: Vec::new() });
        if entry.stamp != now {
            *entry = Seen { stamp: now, ranges: Vec::new() };
        }
        entry.ranges.push((first, last));
    }

    pub fn forget(&self) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.clear();
        }
    }

    pub fn unwatched(&self) -> Unwatched<'_> {
        self.unwatched.fetch_add(1, Ordering::SeqCst);
        Unwatched(self)
    }

    fn is_unwatched(&self) -> bool {
        self.unwatched.load(Ordering::SeqCst) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::SeenFiles;

    #[test]
    fn reads_made_out_of_the_model_s_sight_are_neither_recorded_nor_answered_from_memory() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, "alpha\nbeta\n").expect("write");
        let seen = SeenFiles::default();

        {
            let _script = seen.unwatched();
            seen.saw(&path, 1, 2);
        }
        assert!(!seen.covers(&path, 1, 2), "what a script read the model never saw");

        seen.saw(&path, 1, 2);
        assert!(seen.covers(&path, 1, 2));
        let script = seen.unwatched();
        assert!(!seen.covers(&path, 1, 2), "a script always gets the text");
        drop(script);
        assert!(seen.covers(&path, 1, 2), "the model's own reads are remembered again afterwards");
    }
}
