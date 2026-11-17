use std::collections::HashSet;
use std::path::{Path, PathBuf};

use domain::MemoryBook;
use domain::legacy::OWNER_DIR;
use platform::store::Document;

use super::sqlite;
use crate::db::{Db, imported};

const EXTENSION: &str = "json";

#[derive(Debug, Clone)]
pub(crate) struct LegacyMemoryStore {
    root: PathBuf,
}

#[derive(Debug)]
struct Kept {
    key: String,
    file: PathBuf,
}

impl LegacyMemoryStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn folder(&self) -> PathBuf {
        self.root.join(OWNER_DIR)
    }

    fn kept(&self) -> Vec<Kept> {
        let Ok(listed) = std::fs::read_dir(self.folder()) else { return Vec::new() };
        let mut kept: Vec<Kept> = listed
            .flatten()
            .map(|entry| entry.path())
            .filter(|file| file.extension().is_some_and(|extension| extension == EXTENSION))
            .filter_map(|file| {
                let key = file.file_stem()?.to_str()?.to_owned();
                (!key.starts_with('.') && file.is_file()).then_some(Kept { key, file })
            })
            .collect();
        kept.sort_by(|a, b| a.key.cmp(&b.key));
        kept
    }

    #[cfg(test)]
    pub(crate) fn save(&self, project: &Path, book: &MemoryBook) {
        let file = self.folder().join(format!("{}.{EXTENSION}", crate::dirs::project_key(project)));
        Document::at(file).write(book).expect("legacy memory");
    }
}

fn book(file: &Path) -> Option<MemoryBook> {
    match Document::at(file).read::<MemoryBook>() {
        Ok(book) => book.map(without_repeated_ids),
        Err(error) => {
            tracing::warn!(%error, "the memory of an earlier version could not be read; it was not imported");
            None
        }
    }
}

fn without_repeated_ids(mut book: MemoryBook) -> MemoryBook {
    let held = book.memories.len() + book.forgotten.len();
    let mut seen = HashSet::new();
    book.memories.retain(|memory| seen.insert(memory.id));
    book.forgotten.retain(|forgotten| seen.insert(forgotten.memory.id));
    if seen.len() != held {
        tracing::warn!(dropped = held - seen.len(), "memories repeating another's id were not imported");
    }
    book
}

pub(crate) fn import(db: &Db, folder: &Path) {
    let kept = LegacyMemoryStore::new(folder.to_path_buf()).kept();
    if kept.is_empty() {
        return;
    }
    let imported = db.write(|transaction| {
        let mut imported = Vec::new();
        if !sqlite::is_empty(transaction)? {
            return Ok(imported);
        }
        for Kept { key, file } in &kept {
            let Some(book) = book(file) else { continue };
            sqlite::keep(transaction, key, &sqlite::Stored::default(), &book)?;
            imported.push(file);
        }
        Ok(imported)
    });
    match imported {
        Ok(files) => files.into_iter().for_each(|file| imported::set_aside_or_warn(file)),
        Err(error) => tracing::warn!(%error, "the memory of an earlier version was not imported"),
    }
}
