//! Plugin errors.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    #[error("plugin `{0}` is already installed")]
    AlreadyInstalled(String),
    #[error("plugin `{0}` is not installed")]
    NotFound(String),
    #[error("invalid plugin manifest: {0}")]
    Invalid(String),
    #[error("template error: {0}")]
    Template(String),
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for PluginError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
