use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

type Stamp = (u64, Option<SystemTime>);

#[derive(Debug)]
struct Seen {
    stamp: Stamp,
    ranges: Vec<(usize, usize)>,
}

#[derive(Debug, Default)]
pub struct SeenFiles(Mutex<HashMap<PathBuf, Seen>>);

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

impl SeenFiles {
    pub fn covers(&self, path: &Path, first: usize, last: usize) -> bool {
        let (Some(now), Ok(seen)) = (stamp(path), self.0.lock()) else { return false };
        first <= last
            && seen.get(path).is_some_and(|seen| {
                seen.stamp == now && seen.ranges.iter().any(|(from, to)| *from <= first && last <= *to)
            })
    }

    pub fn saw(&self, path: &Path, first: usize, last: usize) {
        let (Some(now), Ok(mut seen)) = (stamp(path), self.0.lock()) else { return };
        let entry = seen.entry(path.to_path_buf()).or_insert_with(|| Seen { stamp: now, ranges: Vec::new() });
        if entry.stamp != now {
            *entry = Seen { stamp: now, ranges: Vec::new() };
        }
        entry.ranges.push((first, last));
    }

    pub fn forget(&self) {
        if let Ok(mut seen) = self.0.lock() {
            seen.clear();
        }
    }
}
