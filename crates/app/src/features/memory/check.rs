use std::path::Path;

use crate::api::features::memory::MemoryCheck;
use domain::memory::Memory;

use super::{Memories, team};

const MOST_DIFF_BYTES: usize = 200_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    pub base: String,
    pub files: Vec<String>,
    pub diff: String,
}

impl Memories {
    pub fn check(&self, project: &Path) -> Result<MemoryCheck, String> {
        let features = team::features(&self.gate(project));
        if !features.on(domain::features::LEARNING_CHECK) {
            return Err(format!("/check is turned off ({}).", domain::features::LEARNING_CHECK));
        }
        let changes = changes(project)?;
        let shared = team::load_if(project, &features);
        let known: Vec<Memory> = {
            let mut books = self.lock();
            self.book(&mut books, project).memories.iter().cloned().chain(shared).collect()
        };
        let relevant = domain::memory::check::relevant(&known, &changes.files, &changes.diff);
        let prompt = domain::memory::check::prompt(&changes.base, &changes.files, &relevant);
        Ok(MemoryCheck { path: project.to_path_buf(), prompt, changed: changes.files.len(), memories: relevant.len() })
    }
}

pub fn changes(project: &Path) -> Result<Changes, String> {
    git(project, &["rev-parse", "--git-dir"]).map_err(|_| "/check needs a git repository.".to_owned())?;
    let head = git(project, &["rev-parse", "HEAD"]).unwrap_or_default();
    let base = default_branch(project)
        .and_then(|branch| git(project, &["merge-base", "HEAD", &branch]).ok())
        .filter(|base| !base.is_empty() && *base != head)
        .map_or_else(|| "HEAD".to_owned(), |base| base.chars().take(12).collect());
    let base = if head.is_empty() { git_empty_tree(project) } else { base };
    let mut files: Vec<String> = git(project, &["diff", "--no-ext-diff", "--no-textconv", "--name-only", &base])?
        .lines()
        .map(str::to_owned)
        .filter(|f| !f.is_empty())
        .collect();
    for file in git(project, &["ls-files", "--others", "--exclude-standard"])?.lines() {
        if !file.is_empty() && !files.iter().any(|known| known == file) {
            files.push(file.to_owned());
        }
    }
    let mut diff = git(project, &["diff", "--no-ext-diff", "--no-textconv", &base])?;
    if diff.len() > MOST_DIFF_BYTES {
        let cut = (0..=MOST_DIFF_BYTES).rev().find(|&at| diff.is_char_boundary(at)).unwrap_or(0);
        diff.truncate(cut);
    }
    Ok(Changes { base, files, diff })
}

fn default_branch(project: &Path) -> Option<String> {
    let remote = git(project, &["rev-parse", "--abbrev-ref", "origin/HEAD"]).ok().filter(|name| name != "origin/HEAD");
    remote.or_else(|| {
        ["main", "master"]
            .into_iter()
            .find(|branch| git(project, &["rev-parse", "--verify", "--quiet", branch]).is_ok())
            .map(str::to_owned)
    })
}

fn git_empty_tree(project: &Path) -> String {
    git(project, &["hash-object", "-t", "tree", "/dev/null"]).unwrap_or_else(|_| "HEAD".into())
}

fn git(project: &Path, args: &[&str]) -> Result<String, String> {
    crate::git::run(project, args, crate::git::LOCAL).map(|output| output.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let done = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git");
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    }

    #[test]
    fn the_changes_are_those_since_the_branch_left_main_plus_new_files() {
        let dir = tempfile::tempdir().expect("dir");
        run(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("client.go"), "retries = 4\n").expect("write");
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-q", "-m", "one"]);
        run(dir.path(), &["checkout", "-q", "-b", "work"]);
        std::fs::write(dir.path().join("client.go"), "retries = 10\n").expect("write");
        run(dir.path(), &["commit", "-q", "-am", "two"]);
        std::fs::write(dir.path().join("new.go"), "x\n").expect("write");

        let changes = changes(dir.path()).expect("changes");

        assert_ne!(changes.base, "HEAD");
        assert_eq!(changes.files, ["client.go", "new.go"]);
        assert!(changes.diff.contains("+retries = 10"));
    }

    #[test]
    fn on_main_the_changes_are_the_uncommitted_ones_and_outside_git_there_are_none() {
        let dir = tempfile::tempdir().expect("dir");
        assert!(changes(dir.path()).is_err());
        run(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("a.rs"), "a\n").expect("write");
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-q", "-m", "one"]);
        std::fs::write(dir.path().join("a.rs"), "b\n").expect("write");

        let changes = changes(dir.path()).expect("changes");

        assert_eq!((changes.base.as_str(), changes.files.as_slice()), ("HEAD", ["a.rs".to_owned()].as_slice()));
    }

    #[cfg(unix)]
    #[test]
    fn reading_the_changes_runs_nothing_the_repository_s_config_names() {
        let hostile = crate::hostile_repository::Hostile::new();

        let changes = changes(hostile.path()).expect("changes");

        assert!(changes.diff.contains("+two"), "{}", changes.diff);
        assert_eq!(hostile.ran(), None, "the repository's fsmonitor, filter or diff program ran");
        assert!(hostile.plain_git_runs_it(), "the same repository does run them through plain git");
    }
}
