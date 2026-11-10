//! Filesystem setup for a custom agent. Runs under the tool identity.
mod git;

use self::git::{git, git_stopped};
use crate::tools::{Workspace, subagent_config::AgentConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Setup {
    pub cwd: PathBuf,
    pub memory_dir: Option<PathBuf>,
    pub memory: String,
    pub worktree_root: Option<PathBuf>,
    pub context: Option<crate::brief::Brief>,
    pub base_commit: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeState {
    pub cwd: PathBuf,
    pub root: PathBuf,
    pub base_commit: String,
}
impl Setup {
    pub fn worktree_state(&self) -> Option<WorktreeState> {
        Some(WorktreeState {
            cwd: self.cwd.clone(),
            root: self.worktree_root.clone()?,
            base_commit: self.base_commit.clone()?,
        })
    }
}
pub fn local_resume(
    config: &AgentConfig,
    name: &str,
    owner: &str,
    state: &WorktreeState,
    workspace: &Workspace,
) -> Result<Setup, String> {
    let root =
        PathBuf::from(git_stopped(workspace, &workspace.cwd, &["rev-parse".as_ref(), "--show-toplevel".as_ref()])?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
    let expected = state.root.canonicalize().map_err(|e| e.to_string())?;
    let actual =
        state.cwd.canonicalize().map_err(|_| "The session worktree no longer exists; restore it before resuming")?;
    if root != expected
        || actual.parent() != Some(root.join(".superfik/worktrees").as_path())
        || actual.file_name().and_then(|s| s.to_str()).and_then(|s| uuid::Uuid::parse_str(s).ok()).is_none()
    {
        return Err("saved worktree is outside the session project".into());
    }
    let declared = PathBuf::from(git_stopped(workspace, &actual, &["rev-parse".as_ref(), "--show-toplevel".as_ref()])?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if declared != actual {
        return Err("saved worktree is not a registered checkout".into());
    }
    let common = PathBuf::from(git_stopped(workspace, &actual, &["rev-parse".as_ref(), "--git-common-dir".as_ref()])?);
    let common =
        if common.is_absolute() { common } else { actual.join(common) }.canonicalize().map_err(|e| e.to_string())?;
    let expected_common =
        PathBuf::from(git_stopped(workspace, &root, &["rev-parse".as_ref(), "--git-common-dir".as_ref()])?);
    let expected_common = if expected_common.is_absolute() { expected_common } else { root.join(expected_common) }
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if common != expected_common {
        return Err("saved worktree belongs to a different repository".into());
    }
    let mut memory_config = config.clone();
    memory_config.worktree = false;
    let mut setup = local_prepare(&memory_config, name, owner, workspace)?;
    setup.cwd = actual;
    setup.worktree_root = Some(root);
    setup.base_commit = Some(state.base_commit.clone());
    let request = superfik_core::PromptRequest::plain(setup.cwd.clone(), "", workspace.model.clone());
    setup.context = Some(crate::brief::Brief::load(&request)?);
    Ok(setup)
}
pub fn resume(config: &AgentConfig, name: &str, state: &WorktreeState, workspace: &Workspace) -> Result<Setup, String> {
    let owner =
        serde_json::to_string(workspace.credentials.owner.as_ref().unwrap_or(&superfik_core::Principal::LocalOwner))
            .map_err(|e| e.to_string())?;
    local_resume(config, name, &owner, state, workspace)
}
pub fn local_prepare(config: &AgentConfig, name: &str, owner: &str, workspace: &Workspace) -> Result<Setup, String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || "_-:".contains(c)) {
        return Err("agent name must be a filesystem-safe name".into());
    }
    let mut setup = Setup { cwd: workspace.cwd.clone(), ..Default::default() };
    let project = workspace.cwd.ancestors().find(|path| path.join(".git").exists()).unwrap_or(&workspace.cwd);
    if let Some(scope) = &config.memory {
        let base = match scope.as_str() {
            "user" => std::env::home_dir().ok_or("user home unavailable")?.join(".superfik/agent-memory"),
            "project" => project.join(".superfik/agent-memory"),
            "local" => project.join(".superfik/agent-memory-local"),
            _ => return Err("invalid memory scope".into()),
        };
        let owner = format!("{:x}", Sha256::digest(owner.as_bytes()));
        let name_key = format!("{}-{:x}", name.replace(':', "--"), Sha256::digest(name.as_bytes()));
        let dir = base.join(owner).join(name_key);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        }
        let mut bytes = Vec::new();
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        match options.open(dir.join("MEMORY.md")) {
            Ok(file) => {
                if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                    return Err("agent memory index must be a regular file".into());
                }
                file.take(25_000).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        setup.memory = String::from_utf8_lossy(&bytes).lines().take(200).collect::<Vec<_>>().join("\n");
        setup.memory_dir = Some(dir.canonicalize().map_err(|e| e.to_string())?);
    }
    if config.worktree {
        let root =
            PathBuf::from(git_stopped(workspace, &workspace.cwd, &["rev-parse".as_ref(), "--show-toplevel".as_ref()])?);
        let base = git_stopped(workspace, &root, &["symbolic-ref".as_ref(), "refs/remotes/origin/HEAD".as_ref()])
            .or_else(|_| {
                for name in ["main", "master"] {
                    if git_stopped(workspace, &root, &["rev-parse".as_ref(), "--verify".as_ref(), name.as_ref()])
                        .is_ok()
                    {
                        return Ok(name.into());
                    }
                }
                Ok::<_, String>("HEAD".into())
            })?;
        let target = root.join(".superfik/worktrees").join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(target.parent().ok_or("worktree parent missing")?).map_err(|e| e.to_string())?;
        setup.base_commit = Some(git_stopped(workspace, &root, &["rev-parse".as_ref(), base.as_ref()])?);
        setup.cwd = target.clone();
        setup.worktree_root = Some(root.clone());
        if let Err(error) = git_stopped(
            workspace,
            &root,
            &["worktree".as_ref(), "add".as_ref(), "--detach".as_ref(), target.as_os_str(), base.as_ref()],
        ) {
            let cleanup = local_cleanup(&setup);
            return Err(match cleanup {
                Ok(None) => error,
                Ok(Some(path)) => format!("{error}; partial checkout retained at {}", path.display()),
                Err(cleanup) => format!("{error}; inspect partial checkout {}: {cleanup}", target.display()),
            });
        }
        let request = superfik_core::PromptRequest::plain(setup.cwd.clone(), "", workspace.model.clone());
        setup.context = match crate::brief::Brief::load(&request) {
            Ok(brief) => Some(brief),
            Err(error) => {
                let _ = local_cleanup(&setup);
                return Err(error);
            }
        };
    }
    Ok(setup)
}
pub fn local_cleanup(setup: &Setup) -> Result<Option<PathBuf>, String> {
    let Some(root) = &setup.worktree_root else { return Ok(None) };
    if !setup.cwd.starts_with(root.join(".superfik/worktrees"))
        || setup.cwd.file_name().and_then(|s| s.to_str()).and_then(|s| uuid::Uuid::parse_str(s).ok()).is_none()
    {
        return Err("invalid managed worktree".into());
    }
    let head = git(&setup.cwd, &["rev-parse".as_ref(), "HEAD".as_ref()])?;
    if setup.base_commit.as_deref() == Some(head.as_str())
        && git(&setup.cwd, &["status".as_ref(), "--porcelain".as_ref(), "--untracked-files=all".as_ref()])?.is_empty()
    {
        git(root, &["worktree".as_ref(), "remove".as_ref(), setup.cwd.as_os_str()])?;
        Ok(None)
    } else {
        Ok(Some(setup.cwd.clone()))
    }
}
pub fn prepare(config: &AgentConfig, name: &str, workspace: &Workspace) -> Result<Setup, String> {
    if config.memory.is_none() && !config.worktree {
        return Ok(Setup { cwd: workspace.cwd.clone(), ..Default::default() });
    }
    let owner =
        serde_json::to_string(workspace.credentials.owner.as_ref().unwrap_or(&superfik_core::Principal::LocalOwner))
            .map_err(|e| e.to_string())?;
    local_prepare(config, name, &owner, workspace)
}
pub fn cleanup(setup: &Setup) -> Result<Option<PathBuf>, String> {
    if setup.worktree_root.is_none() {
        return Ok(None);
    }
    local_cleanup(setup)
}

mod guard;
pub use guard::Guard;
