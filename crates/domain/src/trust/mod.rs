mod contributions;
mod dialog;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use contributions::{
    Contribution, ContributionKind, Contributions, LEGACY_OPT_IN, NOT_TRUSTED, TrustReport, legacy_notice,
};
pub use dialog::{ASK, DEFAULT_CHOICE, TrustAnswer, answer, ask};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    Trusted,
    #[default]
    Untrusted,
}

impl Trust {
    pub fn given(trusted: bool) -> Self {
        if trusted { Self::Trusted } else { Self::Untrusted }
    }

    pub fn is_trusted(self) -> bool {
        self == Self::Trusted
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    Home,
    FilesystemRoot,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Home => {
                "your home folder cannot be trusted as a project: open the project's own folder and trust that one"
            }
            Self::FilesystemRoot => "the root of the file system cannot be trusted as a project",
        })
    }
}

pub fn refusal(folder: &Path, home: Option<&Path>) -> Option<Refusal> {
    if folder.parent().is_none() {
        Some(Refusal::FilesystemRoot)
    } else if home == Some(folder) {
        Some(Refusal::Home)
    } else {
        None
    }
}

pub fn project_root(folder: &Path, is_repository: &dyn Fn(&Path) -> bool) -> PathBuf {
    folder.ancestors().find(|dir| is_repository(dir)).map_or_else(|| folder.to_path_buf(), Path::to_path_buf)
}

pub fn trust_root(folder: &Path, home: Option<&Path>, is_repository: &dyn Fn(&Path) -> bool) -> PathBuf {
    project_root(folder, &|dir| is_repository(dir) && refusal(dir, home).is_none())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedFolder {
    pub path: PathBuf,
    pub trusted_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrustedFolders(Vec<TrustedFolder>);

impl TrustedFolders {
    pub fn trust(&mut self, root: &Path, home: Option<&Path>, now: u64) -> Result<(), Refusal> {
        if let Some(refused) = refusal(root, home) {
            return Err(refused);
        }
        if self.of_root(root) == Trust::Untrusted {
            self.0.push(TrustedFolder { path: root.to_path_buf(), trusted_at: now });
        }
        Ok(())
    }

    pub fn untrust(&mut self, root: &Path) {
        self.0.retain(|folder| folder.path != root);
    }

    pub fn of_root(&self, root: &Path) -> Trust {
        Trust::given(self.0.iter().any(|folder| folder.path == root))
    }

    pub fn of(&self, folder: &Path, home: Option<&Path>, is_repository: &dyn Fn(&Path) -> bool) -> Trust {
        self.of_root(&trust_root(folder, home, is_repository))
    }

    pub fn listed(&self) -> &[TrustedFolder] {
        &self.0
    }
}

#[cfg(test)]
mod tests;
