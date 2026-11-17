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

const NESTED_GIT_RUNS: [&str; 4] = ["config", "config.worktree", "hooks", "commondir"];
const NESTED_DEPTH: usize = 6;
const NESTED_MOST: usize = 512;

pub fn confinement_of(sandbox: &domain::sandbox::Sandbox, places: &Places<'_>) -> platform::sandbox::Confinement {
    from_domain(sandbox.confinement(places), places)
}

pub fn from_domain(found: domain::sandbox::Confinement, places: &Places<'_>) -> platform::sandbox::Confinement {
    let mut protected: Vec<PathBuf> =
        found.protected.into_iter().filter(|path| repository_of(path).is_none_or(Path::is_dir)).collect();
    let roots =
        std::iter::once(places.project).chain(places.added.iter().map(PathBuf::as_path)).chain(places.repository);
    for root in roots {
        for path in nested_git_runs(root) {
            if !protected.contains(&path) {
                protected.push(path);
            }
        }
    }
    platform::sandbox::Confinement {
        writable: found.writable,
        protected,
        network: found.network,
        environment: found.environment,
        sockets: found.sockets,
        unreadable: found.unreadable,
        readable: found.readable,
    }
}

fn repository_of(path: &Path) -> Option<&Path> {
    path.ancestors().find(|folder| folder.file_name().is_some_and(|name| name == ".git"))
}

fn nested_git_runs(root: &Path) -> Vec<PathBuf> {
    let git = root.join(".git");
    match std::fs::symlink_metadata(&git) {
        Ok(found) if found.is_file() => return vec![git],
        Ok(found) if found.is_dir() => {}
        _ => return Vec::new(),
    }
    let mut guarded = Vec::new();
    for linked in folders_in(&git.join("worktrees")) {
        guarded.extend(["config.worktree", "commondir"].map(|name| linked.join(name)));
    }
    let mut pending = vec![(git.join("modules"), 0)];
    let mut seen = 0;
    while let Some((folder, depth)) = pending.pop() {
        if depth > NESTED_DEPTH || seen > NESTED_MOST {
            break;
        }
        seen += 1;
        if folder.join("HEAD").is_file() {
            guarded.extend(NESTED_GIT_RUNS.map(|name| folder.join(name)));
            pending.push((folder.join("modules"), depth + 1));
        } else {
            pending.extend(folders_in(&folder).into_iter().map(|inner| (inner, depth + 1)));
        }
    }
    guarded
}

fn folders_in(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .take(NESTED_MOST)
        .collect()
}

pub fn data_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> =
        [platform::data_dir(brand::NAME, brand::COMMAND), platform::local_data_dir(brand::NAME, brand::COMMAND)]
            .into_iter()
            .flatten()
            .collect();
    folders.dedup();
    folders
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
        let data = data_folders();
        Some(confinement_of(
            sandbox,
            &Places {
                project: &self.cwd,
                added: &self.added_dirs,
                repository: self.worktree.as_ref().map(|worktree| worktree.root.as_path()),
                temp: &temp,
                home: self.home.as_deref(),
                settings_dir: brand::DIR,
                data: &data,
            },
        ))
    }
}
