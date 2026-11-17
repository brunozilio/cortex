use rusqlite::{Connection, OptionalExtension};

const SESSIONS_AND_MEMORY: &str = "
CREATE TABLE sessions (
    session TEXT PRIMARY KEY NOT NULL,
    cwd TEXT NOT NULL,
    title TEXT,
    created INTEGER NOT NULL,
    updated INTEGER NOT NULL,
    status TEXT NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0,
    unread INTEGER NOT NULL DEFAULT 0,
    tie INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX sessions_newest_first ON sessions (updated DESC, tie);
CREATE INDEX sessions_by_cwd ON sessions (cwd, updated DESC, tie);

CREATE TABLE memory_books (
    project TEXT PRIMARY KEY NOT NULL,
    next INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 0,
    with_runs INTEGER NOT NULL DEFAULT 0,
    with_corrections INTEGER NOT NULL DEFAULT 0,
    with_cost_usd REAL NOT NULL DEFAULT 0,
    without_runs INTEGER NOT NULL DEFAULT 0,
    without_corrections INTEGER NOT NULL DEFAULT 0,
    without_cost_usd REAL NOT NULL DEFAULT 0,
    briefed INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE memories (
    project TEXT NOT NULL REFERENCES memory_books (project) ON DELETE CASCADE,
    id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    forgotten_at INTEGER,
    kind TEXT NOT NULL,
    author TEXT NOT NULL,
    text TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    touches TEXT NOT NULL DEFAULT '[]',
    history TEXT NOT NULL DEFAULT '[]',
    created INTEGER NOT NULL,
    origin TEXT,
    uses INTEGER NOT NULL DEFAULT 0,
    last_used INTEGER NOT NULL DEFAULT 0,
    credit TEXT,
    PRIMARY KEY (project, id)
);

CREATE TABLE memory_undo_points (
    project TEXT PRIMARY KEY NOT NULL,
    book TEXT NOT NULL
);
";

pub(super) const MIGRATIONS: &[&str] = &[SESSIONS_AND_MEMORY];

#[derive(Debug)]
pub(super) enum Refused {
    Sqlite(rusqlite::Error),
    Newer { found: i64, known: i64 },
}

impl From<rusqlite::Error> for Refused {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

pub(super) fn version(connection: &Connection) -> rusqlite::Result<i64> {
    let versioned: Option<String> = connection
        .query_row("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'", [], |row| {
            row.get(0)
        })
        .optional()?;
    if versioned.is_none() {
        return Ok(0);
    }
    let version: Option<i64> = connection.query_row("SELECT max(version) FROM schema_version", [], |row| row.get(0))?;
    Ok(version.unwrap_or(0))
}

pub(super) fn is_current(connection: &Connection, migrations: &[&str]) -> Result<bool, Refused> {
    let (found, known) = (version(connection)?, known(migrations));
    if found > known {
        return Err(Refused::Newer { found, known });
    }
    Ok(found == known)
}

pub(super) fn migrate(connection: &Connection, migrations: &[&str]) -> Result<(), Refused> {
    if is_current(connection, migrations)? {
        return Ok(());
    }
    let found = version(connection)?;
    connection.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    for (step, migration) in migrations.iter().enumerate().skip(usize::try_from(found).unwrap_or(usize::MAX)) {
        connection.execute_batch(migration)?;
        connection.execute("DELETE FROM schema_version", [])?;
        connection.execute("INSERT INTO schema_version (version) VALUES (?1)", [known(&migrations[..=step])])?;
    }
    Ok(())
}

fn known(migrations: &[&str]) -> i64 {
    i64::try_from(migrations.len()).unwrap_or(i64::MAX)
}
