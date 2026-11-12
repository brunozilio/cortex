use std::path::Path;

const STATUS_LINES: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct GitSnapshot {
    pub branch: String,
    pub status: String,
    pub recent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub date: String,
    pub git: Option<GitSnapshot>,
    #[serde(default)]
    pub top_level: Option<Vec<String>>,
}

pub fn snapshot(cwd: &Path) -> Option<GitSnapshot> {
    let never = std::sync::atomic::AtomicBool::new(false);
    let git = |args: &[&str]| -> Option<String> {
        let args: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        crate::agent_runtime::git::output(cwd, &args, &never).ok()
    };
    let branch = git(&["symbolic-ref", "--short", "HEAD"])
        .or_else(|| git(&["rev-parse", "--abbrev-ref", "HEAD"]))?
        .trim()
        .to_owned();
    let status: String =
        git(&["status", "--short"]).unwrap_or_default().lines().take(STATUS_LINES).collect::<Vec<_>>().join("\n");
    let recent = git(&["log", "--oneline", "-5"]).unwrap_or_default();
    Some(GitSnapshot { branch, status, recent })
}
