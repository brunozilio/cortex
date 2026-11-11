use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::tools::Workspace;

#[derive(Debug)]
pub struct Watched {
    root: PathBuf,
    base: String,
    untracked: HashSet<String>,
}

pub fn before(workspace: &Workspace) -> Option<Watched> {
    workspace.files.as_ref()?;
    let root = PathBuf::from(text(&workspace.cwd, &["rev-parse", "--show-toplevel"])?.trim());
    let stashed = text(&root, &["stash", "create"])?.trim().to_owned();
    let base = if stashed.is_empty() { text(&root, &["rev-parse", "HEAD"])?.trim().to_owned() } else { stashed };
    let untracked = changed(&root)?.into_iter().filter(|(code, _)| code == "??").map(|(_, path)| path).collect();
    Some(Watched { root, base, untracked })
}

impl Watched {
    pub fn after(self, workspace: &Workspace) {
        let (Some(files), Some(now)) = (workspace.files.as_ref(), changed(&self.root)) else { return };
        for (_, path) in now.into_iter().filter(|(_, path)| !self.untracked.contains(path)) {
            let before = bytes(&self.root, &["show", &format!("{}:{path}", self.base)]);
            files.record(&self.root.join(&path), before);
        }
    }
}

fn changed(root: &Path) -> Option<Vec<(String, String)>> {
    let listed = bytes(root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
    let mut entries = Vec::new();
    let mut parts = listed.split(|byte| *byte == 0).filter(|part| part.len() > 3);
    while let Some(part) = parts.next() {
        let entry = String::from_utf8_lossy(part);
        let (code, path) = entry.split_at(3);
        let code = code.trim().to_owned();
        if (code.starts_with('R') || code.starts_with('C'))
            && let Some(original) = parts.next()
        {
            entries.push((code.clone(), String::from_utf8_lossy(original).into_owned()));
        }
        entries.push((code, path.to_owned()));
    }
    Some(entries)
}

fn text(cwd: &Path, args: &[&str]) -> Option<String> {
    bytes(cwd, args).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

fn bytes(cwd: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git").args(["-c", "core.quotepath=off"]).args(args).current_dir(cwd).output().ok()?;
    output.status.success().then_some(output.stdout)
}
