//! A Hugging Face model id (`<org>/<name>`), checked before it names a
//! folder: a bare name means `mlx-community/<name>`.

use std::path::{Path, PathBuf};

use super::LocalModelError;

/// Where MLX conversions of open models are published.
pub const DEFAULT_ORG: &str = "mlx-community";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoId {
    org: String,
    name: String,
}

impl RepoId {
    pub fn parse(text: &str) -> Result<Self, LocalModelError> {
        let text = text.trim();
        let invalid = |why: &str| LocalModelError::InvalidId { id: text.to_owned(), why: why.to_owned() };
        if text.is_empty() {
            return Err(invalid("it is empty"));
        }
        if !text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/')) {
            return Err(invalid("only letters, digits, `.`, `_`, `-` and one `/` are allowed"));
        }
        if text.contains("..") {
            return Err(invalid("`..` is not allowed"));
        }
        let (org, name) = match text.split_once('/') {
            Some((org, name)) => (org, name),
            None => (DEFAULT_ORG, text),
        };
        if name.contains('/') {
            return Err(invalid("it has more than one `/`"));
        }
        if org.is_empty() || name.is_empty() {
            return Err(invalid("it needs both an organisation and a name"));
        }
        if org.starts_with('.') || name.starts_with('.') {
            return Err(invalid("a part may not start with `.`"));
        }
        Ok(Self { org: org.to_owned(), name: name.to_owned() })
    }

    /// Where the model lives once downloaded: `<root>/<org>/<name>`.
    pub fn folder(&self, root: &Path) -> PathBuf {
        root.join(&self.org).join(&self.name)
    }

    /// Where a download is put together: hidden next to [`Self::folder`],
    /// so a model is never listed before it is complete.
    pub fn partial(&self, root: &Path) -> PathBuf {
        root.join(&self.org).join(format!(".{}.partial", self.name))
    }
}

impl std::fmt::Display for RepoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.org, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> String {
        RepoId::parse(text).map(|id| id.to_string()).unwrap_or_else(|e| format!("error: {e}"))
    }

    #[test]
    fn a_full_id_is_kept_and_a_bare_name_is_mlx_communitys() {
        assert_eq!(id("mlx-community/Qwen3-8B-4bit"), "mlx-community/Qwen3-8B-4bit");
        assert_eq!(id("  Qwen3-8B-4bit "), "mlx-community/Qwen3-8B-4bit");
        assert_eq!(id("lmstudio_community/gemma-3.1_it"), "lmstudio_community/gemma-3.1_it");
    }

    #[test]
    fn ids_that_could_leave_the_models_folder_are_refused() {
        for bad in [
            "",
            "../etc",
            "org/..",
            "org/a..b",
            "/abs/path",
            "/name",
            "org/",
            "a/b/c",
            "org/.hidden",
            ".org/name",
            "org\\name",
            "org/na me",
            "https://huggingface.co/org/name",
            "org/näme",
        ] {
            assert!(RepoId::parse(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn the_partial_download_is_hidden_beside_the_model() {
        let id = RepoId::parse("mlx-community/Qwen3-8B-4bit").expect("id");
        let root = Path::new("/models");
        assert_eq!(id.folder(root), Path::new("/models/mlx-community/Qwen3-8B-4bit"));
        assert_eq!(id.partial(root), Path::new("/models/mlx-community/.Qwen3-8B-4bit.partial"));
    }
}
