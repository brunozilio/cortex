use std::io;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{place}: {source}")]
    Sqlite {
        place: String,
        #[source]
        source: rusqlite::Error,
    },
    #[error("{place} is being written by another process; gave up after {} s", waited.as_secs_f32())]
    Busy { place: String, waited: Duration },
    #[error(
        "{place} was written by a newer version (schema {found}, this version knows up to {known}); it was left as it is"
    )]
    Newer { place: String, found: i64, known: i64 },
    #[error("{place}: {source}")]
    Io {
        place: String,
        #[source]
        source: io::Error,
    },
}

impl Error {
    pub(super) fn of(place: &str, waited: Duration, source: rusqlite::Error) -> Self {
        if is_busy(&source) {
            return Self::Busy { place: place.to_owned(), waited };
        }
        Self::Sqlite { place: place.to_owned(), source }
    }
}

pub(super) fn is_busy(error: &rusqlite::Error) -> bool {
    use rusqlite::ErrorCode::{DatabaseBusy, DatabaseLocked};
    matches!(error, rusqlite::Error::SqliteFailure(failure, _) if matches!(failure.code, DatabaseBusy | DatabaseLocked))
}

impl From<Error> for io::Error {
    fn from(error: Error) -> Self {
        let kind = match &error {
            Error::Io { source, .. } => source.kind(),
            Error::Busy { .. } => io::ErrorKind::TimedOut,
            Error::Sqlite { .. } | Error::Newer { .. } => io::ErrorKind::InvalidData,
        };
        io::Error::new(kind, error.to_string())
    }
}
