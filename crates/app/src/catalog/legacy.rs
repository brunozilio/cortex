use std::path::{Path, PathBuf};

use domain::catalog::Catalog;
use domain::legacy::OWNER_DIR;
use platform::store::Document;

use super::sqlite;
use crate::db::{Db, imported};

#[derive(Debug)]
pub(crate) struct LegacySessionCatalog {
    kept: Document,
}

impl LegacySessionCatalog {
    pub(crate) fn new(users: PathBuf) -> Self {
        Self { kept: Document::at(users.join(OWNER_DIR).join("sessions.json")).compact() }
    }

    fn file(&self) -> &Path {
        self.kept.path()
    }

    fn listed(&self) -> Option<Catalog> {
        match self.kept.read() {
            Ok(catalog) => catalog,
            Err(error) => {
                tracing::warn!(%error, "the session list of an earlier version could not be read; it was not imported");
                None
            }
        }
    }
}

#[cfg(test)]
impl super::SessionCatalog for LegacySessionCatalog {
    fn read(&self) -> Catalog {
        self.kept.read_or_default()
    }

    fn update(&self, change: &mut dyn FnMut(&mut Catalog)) {
        self.kept.update(|catalog: &mut Catalog| change(catalog)).expect("legacy catalog");
    }
}

pub(crate) fn import(db: &Db, users: &Path) {
    let legacy = LegacySessionCatalog::new(users.to_path_buf());
    if !legacy.file().exists() {
        return;
    }
    let imported = db.write(|transaction| {
        if !sqlite::is_empty(transaction)? {
            return Ok(false);
        }
        let Some(catalog) = legacy.listed() else { return Ok(false) };
        sqlite::keep(transaction, &[], &catalog)?;
        Ok(true)
    });
    match imported {
        Ok(true) => imported::set_aside_or_warn(legacy.file()),
        Ok(false) => {}
        Err(error) => tracing::warn!(%error, "the session list of an earlier version was not imported"),
    }
}
