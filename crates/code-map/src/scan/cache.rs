use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::FileStat;
use crate::extract::FileSymbols;

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    entries: Vec<(FileStat, FileSymbols)>,
}

pub(super) fn load(cache: &Path) -> HashMap<FileStat, FileSymbols> {
    let Ok(bytes) = std::fs::read(cache) else {
        return HashMap::new();
    };
    match serde_json::from_slice::<Stored>(&bytes) {
        Ok(stored) if stored.version == VERSION => stored.entries.into_iter().collect(),
        _ => HashMap::new(),
    }
}

pub(super) fn store(cache: &Path, entries: &[(FileStat, FileSymbols)]) {
    let _ = try_store(cache, entries);
}

fn try_store(cache: &Path, entries: &[(FileStat, FileSymbols)]) -> std::io::Result<()> {
    let directory = cache.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(directory)?;
    let stored = StoredRef { version: VERSION, entries };
    let mut temp = tempfile_in(directory, cache)?;
    let written = serde_json::to_writer(std::io::BufWriter::new(&mut temp.1), &stored)
        .map_err(std::io::Error::other)
        .and_then(|()| temp.1.flush());
    if let Err(error) = written.and_then(|()| std::fs::rename(&temp.0, cache)) {
        let _ = std::fs::remove_file(&temp.0);
        return Err(error);
    }
    Ok(())
}

#[derive(Serialize)]
struct StoredRef<'a> {
    version: u32,
    entries: &'a [(FileStat, FileSymbols)],
}

fn tempfile_in(directory: &Path, cache: &Path) -> std::io::Result<(std::path::PathBuf, std::fs::File)> {
    let name = cache.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let nanos =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_nanos());
    let path = directory.join(format!(".{name}.{}.{nanos}.tmp", std::process::id()));
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    Ok((path, file))
}
