//! The repository as the run starts: branch, changes and recent commits,
//! read once with `git` and described in the prompt.

use std::path::Path;

const STATUS_LINES: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct GitSnapshot {
    pub branch: String,
    pub status: String,
    pub recent: String,
}

impl GitSnapshot {
    pub fn describe(&self) -> String {
        let mut out = String::from(
            "\n# Git\nThe repository's state when the conversation started; it is not updated as you work.\n",
        );
        out.push_str(&format!("- Current branch: {}\n", self.branch));
        let status = if self.status.trim().is_empty() { "(clean)" } else { self.status.trim_end() };
        out.push_str(&format!("\nStatus:\n{status}\n"));
        if !self.recent.trim().is_empty() {
            out.push_str(&format!("\nRecent commits:\n{}\n", self.recent.trim_end()));
        }
        out
    }
}

pub fn snapshot(cwd: &Path) -> Option<GitSnapshot> {
    let never = std::sync::atomic::AtomicBool::new(false);
    let git = |args: &[&str]| -> Option<String> {
        let args: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        crate::agent_runtime::git::output(cwd, &args, &never).ok()
    };
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_owned();
    let status: String =
        git(&["status", "--short"]).unwrap_or_default().lines().take(STATUS_LINES).collect::<Vec<_>>().join("\n");
    let recent = git(&["log", "--oneline", "-5"]).unwrap_or_default();
    Some(GitSnapshot { branch, status, recent })
}
