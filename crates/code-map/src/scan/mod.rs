mod cache;
mod disk;

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::extract::{FileSymbols, extract};
use crate::map::CodeMap;

pub use disk::DiskSource;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileStat {
    pub path: String,
    pub len: u64,
    pub modified: i64,
}

pub trait Source: Sync {
    fn files(&self) -> Vec<FileStat>;
    fn read(&self, path: &str) -> Option<String>;
}

pub fn refresh(root: &Path, cache: &Path) -> CodeMap {
    refresh_with(&DiskSource::new(root), cache)
}

pub fn refresh_with(source: &dyn Source, cache: &Path) -> CodeMap {
    let mut known: HashMap<FileStat, FileSymbols> = cache::load(cache);
    let stats = source.files();
    let unchanged = stats.len() == known.len() && stats.iter().all(|stat| known.contains_key(stat));
    let stale: Vec<&FileStat> = stats.iter().filter(|stat| !known.contains_key(*stat)).collect();
    let fresh = extract_all(source, &stale);
    let mut entries: Vec<(FileStat, FileSymbols)> = Vec::with_capacity(stats.len());
    for stat in stats {
        let symbols = match known.remove(&stat) {
            Some(symbols) => symbols,
            None => match fresh.get(&stat.path) {
                Some(symbols) => symbols.clone(),
                None => continue,
            },
        };
        entries.push((stat, symbols));
    }
    if !unchanged {
        cache::store(cache, &entries);
    }
    CodeMap::new(entries.into_iter().map(|(_, symbols)| symbols).collect())
}

fn extract_all(source: &dyn Source, stale: &[&FileStat]) -> HashMap<String, FileSymbols> {
    let workers = std::thread::available_parallelism().map_or(1, usize::from).min(stale.len().max(1));
    let chunk = stale.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = stale
            .chunks(chunk)
            .map(|batch| {
                scope.spawn(move || batch.iter().filter_map(|stat| extract_one(source, stat)).collect::<Vec<_>>())
            })
            .collect();
        handles.into_iter().filter_map(|handle| handle.join().ok()).flatten().collect()
    })
}

fn extract_one(source: &dyn Source, stat: &FileStat) -> Option<(String, FileSymbols)> {
    let text = source.read(&stat.path)?;
    let symbols =
        extract(&stat.path, &text).unwrap_or_else(|| FileSymbols { path: stat.path.clone(), ..FileSymbols::default() });
    Some((stat.path.clone(), symbols))
}
