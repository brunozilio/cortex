use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::git::Git;
use crate::tools::Workspace;

const FILE_AT_MOST: u64 = 10 * 1024 * 1024;
const SHOWN_AT_MOST: usize = 10 * 1024 * 1024 + 1;
const HELD_AT_MOST: u64 = 64 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
enum Held {
    Was(Option<Vec<u8>>),
    NotKept,
}

#[derive(Debug)]
pub struct Watched {
    root: PathBuf,
    head: Option<String>,
    edited: HashMap<String, Held>,
    untracked: HashSet<String>,
}

pub fn before(workspace: &Workspace) -> Option<Watched> {
    workspace.services.files.as_ref()?;
    let git = Git::stopped_by(&workspace.stop);
    let root = PathBuf::from(git.text(&workspace.cwd, &["rev-parse", "--show-toplevel"])?.trim());
    let head = git.text(&root, &["rev-parse", "--verify", "--quiet", "HEAD"]).map(|head| head.trim().to_owned());
    let (untracked, tracked): (Vec<_>, Vec<_>) = changed(&git, &root)?.into_iter().partition(|(code, _)| code == "??");
    let mut room = HELD_AT_MOST;
    let mut edited = HashMap::new();
    for (_, path) in tracked {
        let held = held(&root.join(&path), &mut room);
        edited.insert(path, held);
    }
    Some(Watched { root, head, edited, untracked: untracked.into_iter().map(|(_, path)| path).collect() })
}

fn held(path: &Path, room: &mut u64) -> Held {
    let size = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => meta.len(),
        Ok(_) => return Held::NotKept,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Held::Was(None),
        Err(_) => return Held::NotKept,
    };
    if size > FILE_AT_MOST || size > *room {
        return Held::NotKept;
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            *room = room.saturating_sub(bytes.len() as u64);
            Held::Was(Some(bytes))
        }
        Err(_) => Held::NotKept,
    }
}

impl Watched {
    pub fn after(self, workspace: &Workspace) {
        let git = Git::stopped_by(&workspace.stop);
        let (Some(files), Some(now)) = (workspace.services.files.as_ref(), changed(&git, &self.root)) else { return };
        let mut found = Vec::new();
        for (path, held) in &self.edited {
            let Held::Was(before) = held else { continue };
            let file = self.root.join(path);
            if std::fs::read(&file).ok() != *before {
                found.push((file, before.clone()));
            }
        }
        let fresh = now.into_iter().map(|(_, path)| path);
        for path in fresh.filter(|path| !self.untracked.contains(path) && !self.edited.contains_key(path)) {
            match committed(&git, &self.root, self.head.as_deref(), &path) {
                Committed::Was(before) => found.push((self.root.join(&path), before)),
                Committed::Unknown => tracing::warn!(%path, "git could not say what this file was before the command"),
            }
        }
        files.record_all(found);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Committed {
    Was(Option<Vec<u8>>),
    Unknown,
}

fn committed(git: &Git<'_>, root: &Path, head: Option<&str>, path: &str) -> Committed {
    let Some(head) = head else { return Committed::Was(None) };
    let Some(listed) = git.bytes(root, &["ls-tree", "-z", "--name-only", head, "--", path], SHOWN_AT_MOST) else {
        return Committed::Unknown;
    };
    if listed.is_empty() {
        return Committed::Was(None);
    }
    match git.bytes(root, &["show", &format!("{head}:{path}")], SHOWN_AT_MOST) {
        Some(bytes) => Committed::Was(Some(bytes)),
        None => Committed::Unknown,
    }
}

fn changed(git: &Git<'_>, root: &Path) -> Option<Vec<(String, String)>> {
    git.status(root, &[]).map(|listed| entries(&listed))
}

fn entries(listed: &[u8]) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut parts = listed.split(|byte| *byte == 0);
    while let Some(part) = parts.next() {
        if part.len() <= 3 {
            continue;
        }
        let (code, path) = part.split_at(3);
        let code = String::from_utf8_lossy(code).trim().to_owned();
        if (code.starts_with('R') || code.starts_with('C'))
            && let Some(original) = parts.next()
        {
            entries.push((code.clone(), String::from_utf8_lossy(original).into_owned()));
        }
        entries.push((code, String::from_utf8_lossy(path).into_owned()));
    }
    entries
}

#[cfg(test)]
#[cfg(unix)]
#[path = "changes_tests.rs"]
mod tests;
