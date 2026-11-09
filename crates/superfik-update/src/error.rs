//! What can go wrong while updating, and how far an update has come.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Checking,
    Starting,
    Downloading { done: u64, total: u64 },
    Verifying,
    Installing,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("the update service could not be reached: {0}")]
    Unreachable(String),
    #[error("the update service refused: {0}")]
    Refused(String),
    #[error("the download is damaged")]
    Corrupt,
    #[error("the update is not signed with Superfik's update key")]
    Forged,
    #[error("the update could not be installed: {0}")]
    Install(String),
    #[error("this install cannot update itself")]
    Unsupported,
}

impl UpdateError {
    pub(crate) fn install(error: impl std::fmt::Display) -> Self {
        Self::Install(error.to_string())
    }
}
