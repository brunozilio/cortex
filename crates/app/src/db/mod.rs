pub(crate) mod codec;
mod error;
pub(crate) mod imported;
mod schema;
#[cfg(test)]
mod tests;

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use platform::sync::Locked;
use rusqlite::{Connection, Transaction, TransactionBehavior};

pub use error::Error;
use schema::Refused;

pub const FILE: &str = "app.db";
pub const WAIT: Duration = Duration::from_secs(10);
const RETRY_EVERY: Duration = Duration::from_millis(10);

#[derive(Clone)]
pub struct Db {
    kept: Arc<Kept>,
}

struct Kept {
    connection: Mutex<Connection>,
    path: Option<PathBuf>,
    place: String,
    wait: Duration,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").field("place", &self.kept.place).finish_non_exhaustive()
    }
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::opened(path, WAIT, schema::MIGRATIONS)
    }

    pub fn in_memory() -> Result<Self, Error> {
        let place = "the database in memory".to_owned();
        let connection =
            Connection::open_in_memory().map_err(|source| Error::Sqlite { place: place.clone(), source })?;
        Self::ready(Kept { connection: Mutex::new(connection), path: None, place, wait: WAIT }, schema::MIGRATIONS)
    }

    pub(crate) fn opened(path: &Path, wait: Duration, migrations: &[&str]) -> Result<Self, Error> {
        let place = path.display().to_string();
        made_private(path).map_err(|source| Error::Io { place: place.clone(), source })?;
        let connection = Connection::open(path).map_err(|source| Error::Sqlite { place: place.clone(), source })?;
        Self::ready(
            Kept { connection: Mutex::new(connection), path: Some(path.to_path_buf()), place, wait },
            migrations,
        )
    }

    fn ready(kept: Kept, migrations: &[&str]) -> Result<Self, Error> {
        let db = Self { kept: Arc::new(kept) };
        db.configured().map_err(|source| db.error(source))?;
        db.migrated(migrations)?;
        Ok(db)
    }

    fn configured(&self) -> rusqlite::Result<()> {
        let connection = self.kept.connection.locked();
        connection.busy_timeout(self.kept.wait)?;
        settled(self.kept.wait, || connection.pragma_update(None, "journal_mode", "WAL"))?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")
    }

    fn migrated(&self, migrations: &[&str]) -> Result<(), Error> {
        let current = {
            let connection = self.kept.connection.locked();
            schema::is_current(&connection, migrations)
        };
        let migrated = match current {
            Ok(true) => Ok(()),
            Ok(false) => self.within(|transaction| schema::migrate(transaction, migrations)),
            Err(refused) => Err(refused),
        };
        migrated.map_err(|refused| match refused {
            Refused::Sqlite(source) => self.error(source),
            Refused::Newer { found, known } => Error::Newer { place: self.kept.place.clone(), found, known },
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.kept.path.as_deref()
    }

    pub(crate) fn read<R>(&self, query: impl FnOnce(&Connection) -> rusqlite::Result<R>) -> Result<R, Error> {
        let connection = self.kept.connection.locked();
        let snapshot = connection.unchecked_transaction().map_err(|source| self.error(source))?;
        query(&snapshot).map_err(|source| self.error(source))
    }

    pub(crate) fn write<R>(&self, change: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<R>) -> Result<R, Error> {
        self.within(change).map_err(|source| self.error(source))
    }

    fn within<R, E: From<rusqlite::Error>>(
        &self,
        change: impl FnOnce(&Transaction<'_>) -> Result<R, E>,
    ) -> Result<R, E> {
        let mut connection = self.kept.connection.locked();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let answer = change(&transaction)?;
        transaction.commit()?;
        Ok(answer)
    }

    fn error(&self, source: rusqlite::Error) -> Error {
        Error::of(&self.kept.place, self.kept.wait, source)
    }
}

fn made_private(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    platform::private_file::open(OpenOptions::new().write(true).create(true).truncate(false), path).map(drop)
}

fn settled<T>(wait: Duration, mut attempt: impl FnMut() -> rusqlite::Result<T>) -> rusqlite::Result<T> {
    let give_up = Instant::now() + wait;
    loop {
        match attempt() {
            Err(error) if error::is_busy(&error) && Instant::now() < give_up => std::thread::sleep(RETRY_EVERY),
            settled => return settled,
        }
    }
}
