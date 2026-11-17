use std::io;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} did not parse ({reason}); it was kept at {}", path.display(), kept_at.display())]
    Corrupt { path: PathBuf, kept_at: PathBuf, reason: String },
    #[error("{}: {reason}", path.display())]
    Invalid { path: PathBuf, reason: String },
    #[error("{}: {source}", path.display())]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{} is being written by another process; gave up after {} s", path.display(), waited.as_secs_f32())]
    Busy { path: PathBuf, waited: Duration },
}

impl From<Error> for io::Error {
    fn from(error: Error) -> Self {
        let kind = match &error {
            Error::Io { source, .. } => source.kind(),
            Error::Corrupt { .. } | Error::Invalid { .. } | Error::Serialize { .. } => io::ErrorKind::InvalidData,
            Error::Busy { .. } => io::ErrorKind::TimedOut,
        };
        io::Error::new(kind, error.to_string())
    }
}
