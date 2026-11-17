use std::path::PathBuf;
use std::process::Command;

use domain::sandbox::Places;

use super::Workspace;

#[derive(Debug, Clone, Default)]
pub struct Confiner {
    launch: super::Launch,
    confinement: Option<platform::sandbox::Confinement>,
}

impl Confiner {
    pub fn confined(&self, command: Command) -> Command {
        self.launch.0.launch(command, self.confinement.as_ref())
    }
}

impl Workspace {
    pub fn confiner(&self) -> Confiner {
        Confiner { launch: self.services.launch.clone(), confinement: self.confinement() }
    }

    pub fn confined(&self, command: Command) -> Command {
        self.services.launch.0.launch(command, self.confinement().as_ref())
    }

    pub fn sandboxed(&self) -> bool {
        platform::sandbox::SUPPORTED && self.policy.sandbox.is_some()
    }

    pub fn confinement(&self) -> Option<platform::sandbox::Confinement> {
        let sandbox = self.policy.sandbox.as_ref()?;
        let temp = [std::env::temp_dir(), PathBuf::from("/tmp")];
        let found = sandbox.confinement(&Places {
            project: &self.cwd,
            added: &self.added_dirs,
            repository: self.worktree.as_ref().map(|worktree| worktree.root.as_path()),
            temp: &temp,
            home: self.home.as_deref(),
            settings_dir: brand::DIR,
        });
        Some(platform::sandbox::Confinement {
            writable: found.writable,
            protected: found.protected,
            network: found.network,
        })
    }
}
