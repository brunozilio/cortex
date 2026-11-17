mod scan;

use std::path::{Path, PathBuf};

use domain::Folder;
use domain::trust::{Trust, TrustReport};

use crate::tools::settled;

pub use scan::contributions;

fn is_repository(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn canonical(folder: &Path) -> PathBuf {
    settled(folder)
}

pub fn root_of(cwd: &Path, home: Option<&Path>) -> PathBuf {
    let home = home.map(canonical);
    domain::trust::trust_root(&canonical(cwd), home.as_deref(), &is_repository)
}

pub fn report(cwd: &Path, home: Option<&Path>, trust: Trust) -> TrustReport {
    let gate = Gate::at(cwd, home, trust);
    let refusal = domain::trust::refusal(&gate.root, home.map(canonical).as_deref());
    TrustReport { root: gate.root.clone(), trust: gate.trust, refusal, brings: contributions(&gate) }
}

#[derive(Debug, Clone)]
pub struct Gate {
    cwd: PathBuf,
    home: Option<PathBuf>,
    user: Option<PathBuf>,
    root: PathBuf,
    trust: Trust,
}

impl Gate {
    pub fn at(cwd: &Path, home: Option<&Path>, trust: Trust) -> Self {
        let root = root_of(cwd, home);
        let refused = domain::trust::refusal(&root, home.map(settled).as_deref()).is_some();
        Self {
            cwd: cwd.to_path_buf(),
            home: home.map(Path::to_path_buf),
            user: home.map(|home| settled(&home.join(brand::DIR))),
            root,
            trust: if refused { Trust::Untrusted } else { trust },
        }
    }

    pub fn of(folder: Folder<'_>) -> Self {
        Self::within(folder, folder.home())
    }

    pub fn within(folder: Folder<'_>, home: Option<&Path>) -> Self {
        let trust = folder.extensions.map(|extensions| extensions.trust).unwrap_or_default();
        Self::at(folder.cwd, home, trust)
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn trust(&self) -> Trust {
        self.trust
    }

    pub fn is_the_user_s(&self, file: &Path) -> bool {
        let file = settled(file);
        self.user.as_deref().is_some_and(|user| file.starts_with(user))
    }

    pub fn is_the_project_s(&self, file: &Path) -> bool {
        !self.is_the_user_s(file) && settled(file).starts_with(&self.root)
    }

    pub fn honours(&self, file: &Path) -> bool {
        self.is_the_user_s(file) || (self.trust.is_trusted() && self.is_the_project_s(file))
    }

    pub fn project_folders(&self) -> Vec<PathBuf> {
        settled(&self.cwd)
            .ancestors()
            .take_while(|dir| dir.starts_with(&self.root))
            .map(|dir| dir.join(brand::DIR))
            .filter(|folder| !self.is_the_user_s(folder))
            .collect()
    }

    pub fn skipped_notice(&self) -> Option<String> {
        if self.trust.is_trusted() {
            return None;
        }
        contributions(self).skipped_notice()
    }
}

#[cfg(test)]
pub(crate) fn trusted(cwd: &Path, home: &Path) -> Gate {
    Gate::at(cwd, Some(home), Trust::Trusted)
}

#[cfg(test)]
pub(crate) fn untrusted(cwd: &Path, home: &Path) -> Gate {
    Gate::at(cwd, Some(home), Trust::Untrusted)
}

#[cfg(test)]
mod tests;
