use std::path::{Path, PathBuf};
use std::process::Command;

use domain::sandbox::Places;

use super::Workspace;

#[derive(Debug, Clone, Default)]
pub struct Confiner {
    launch: super::Launch,
    confinement: Option<platform::sandbox::Confinement>,
}

impl Confiner {
    pub fn within(confinement: Option<platform::sandbox::Confinement>) -> Self {
        Self { launch: super::Launch::default(), confinement }
    }

    pub fn confined(&self, command: Command) -> Command {
        self.launch.0.launch(command, self.confinement.as_ref())
    }
}

pub fn confinement_of(sandbox: &domain::sandbox::Sandbox, places: &Places<'_>) -> platform::sandbox::Confinement {
    let found = sandbox.confinement(places);
    let protected = found.protected.into_iter().filter(|path| repository_of(path).is_none_or(Path::is_dir)).collect();
    platform::sandbox::Confinement {
        writable: found.writable,
        protected,
        network: found.network,
        environment: found.environment,
        sockets: found.sockets,
    }
}

fn repository_of(path: &Path) -> Option<&Path> {
    path.ancestors().find(|folder| folder.file_name().is_some_and(|name| name == ".git"))
}

pub fn temp_folders() -> [PathBuf; 2] {
    [std::env::temp_dir(), PathBuf::from("/tmp")]
}

impl Workspace {
    pub fn confiner(&self) -> Confiner {
        Confiner { launch: self.services.launch.clone(), confinement: self.confinement() }
    }

    pub fn confined(&self, command: Command) -> Command {
        self.services.launch.0.launch(command, self.confinement().as_ref())
    }

    pub fn host_judge(&self) -> std::sync::Arc<crate::tools::host_judge::HostJudge> {
        self.services.judge.clone().unwrap_or_default()
    }

    fn unavailable(&self) -> Option<String> {
        match self.services.launch.0.support() {
            platform::sandbox::Support::Available => None,
            platform::sandbox::Support::Missing(why) => Some(why),
        }
    }

    pub fn sandboxed(&self) -> bool {
        self.policy.sandbox.is_some() && self.unavailable().is_none()
    }

    pub fn sandbox_missing(&self) -> Option<domain::sandbox::Missing> {
        self.policy.sandbox.as_ref()?.missing(self.unavailable().as_deref())
    }

    pub fn sandbox_refusal(&self) -> Option<String> {
        self.policy.sandbox.as_ref().filter(|sandbox| sandbox.required)?;
        self.unavailable()
    }

    pub fn confinement(&self) -> Option<platform::sandbox::Confinement> {
        let sandbox = self.policy.sandbox.as_ref()?;
        let temp = temp_folders();
        Some(confinement_of(
            sandbox,
            &Places {
                project: &self.cwd,
                added: &self.added_dirs,
                repository: self.worktree.as_ref().map(|worktree| worktree.root.as_path()),
                temp: &temp,
                home: self.home.as_deref(),
                settings_dir: brand::DIR,
            },
        ))
    }
}
