use std::path::{Path, PathBuf};

use domain::destructive::Destroys;

use super::git::Git;
use crate::tools::Workspace;

const MOST_FILES: usize = 5_000;
const MOST_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Kept {
    pub files: usize,
    pub left_out: usize,
}

impl Kept {
    pub fn note(&self) -> Option<String> {
        (self.left_out > 0).then(|| {
            format!(
                "(before this command {} files were kept for /rewind; {} more were too many or too large to keep)",
                self.files, self.left_out
            )
        })
    }
}

pub fn keep(workspace: &Workspace, command: &str) -> Kept {
    let Some(files) = workspace.services.files.as_ref().filter(|_| workspace.policy.snapshots) else {
        return Kept::default();
    };
    let destroys = domain::destructive::destroys(command);
    if !destroys.anything() {
        return Kept::default();
    }
    let mut found = Vec::new();
    for path in &destroys.paths {
        walk(&workspace.resolve(path), &mut found);
    }
    found.extend(outside_the_index(&Git::stopped_by(&workspace.stop), &workspace.cwd, &destroys));
    let mut kept = Kept::default();
    let mut bytes = 0;
    let mut keeping = Vec::new();
    for path in found {
        let size = std::fs::symlink_metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        if kept.files >= MOST_FILES || bytes + size > MOST_BYTES {
            kept.left_out += 1;
            continue;
        }
        bytes += size;
        kept.files += 1;
        keeping.push(path);
    }
    files.before_writes(&keeping);
    kept
}

fn walk(path: &Path, found: &mut Vec<PathBuf>) {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return };
    if meta.is_dir() {
        for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
            walk(&entry.path(), found);
        }
    } else if meta.is_file() {
        found.push(path.to_path_buf());
    }
}

fn outside_the_index(git: &Git<'_>, cwd: &Path, destroys: &Destroys) -> Vec<PathBuf> {
    if !destroys.untracked {
        return Vec::new();
    }
    let Some(root) = git.text(cwd, &["rev-parse", "--show-toplevel"]) else { return Vec::new() };
    let root = PathBuf::from(root.trim());
    let ignored: &[&str] = if destroys.ignored { &["--ignored=matching"] } else { &[] };
    let Some(listed) = git.status(&root, ignored) else { return Vec::new() };
    let entries = listed.split(|byte| *byte == 0).filter(|entry| entry.len() > 3);
    let outside = entries.filter(|entry| entry.starts_with(b"?? ") || entry.starts_with(b"!! "));
    let mut found = Vec::new();
    for entry in outside {
        walk(&root.join(String::from_utf8_lossy(&entry[3..]).as_ref()), &mut found);
    }
    found
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn workspace(dir: &Path) -> Workspace {
        let mut workspace = Workspace::new(dir);
        workspace.services.files = Some(Arc::default());
        workspace.policy.snapshots = true;
        workspace
    }

    fn kept(workspace: &Workspace) -> Vec<String> {
        let files = workspace.services.files.as_ref().expect("files").take().files;
        let mut names: Vec<String> = files.keys().map(|path| path.display().to_string()).collect();
        names.sort();
        names
    }

    #[test]
    fn a_recursive_removal_keeps_every_file_it_will_remove() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir_all(dir.path().join("build/deep")).expect("dirs");
        std::fs::write(dir.path().join("build/a.txt"), "a").expect("a");
        std::fs::write(dir.path().join("build/deep/b.txt"), "b").expect("b");
        std::fs::write(dir.path().join("other.txt"), "o").expect("o");
        let workspace = workspace(dir.path());

        assert_eq!(keep(&workspace, "rm -rf build"), Kept { files: 2, left_out: 0 });

        let names = kept(&workspace);
        assert_eq!(names.len(), 2);
        assert!(names[0].ends_with("build/a.txt") && names[1].ends_with("build/deep/b.txt"), "{names:?}");
    }

    #[test]
    fn git_clean_keeps_untracked_files_and_ignored_ones_only_when_asked() {
        let dir = tempfile::tempdir().expect("dir");
        let never = std::sync::atomic::AtomicBool::new(false);
        let run = |args: &[&str]| assert!(Git::stopped_by(&never).bytes(dir.path(), args, 4096).is_some(), "{args:?}");
        run(&["init", "-q"]);
        std::fs::write(dir.path().join(".gitignore"), "cache/\n").expect("ignore");
        std::fs::create_dir(dir.path().join("cache")).expect("cache");
        std::fs::write(dir.path().join("cache/big.bin"), "x").expect("bin");
        std::fs::write(dir.path().join("draft.md"), "notes").expect("draft");
        let workspace = workspace(dir.path());

        keep(&workspace, "git clean -fd");
        let names = kept(&workspace);
        assert!(names.iter().any(|name| name.ends_with("draft.md")), "{names:?}");
        assert!(!names.iter().any(|name| name.ends_with("big.bin")), "{names:?}");

        keep(&workspace, "git clean -fdx");
        assert!(kept(&workspace).iter().any(|name| name.ends_with("cache/big.bin")));
    }

    #[test]
    fn nothing_is_kept_for_an_ordinary_command_or_with_the_flag_off() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "a").expect("a");
        let mut workspace = workspace(dir.path());
        assert_eq!(keep(&workspace, "cat a.txt"), Kept::default());
        workspace.policy.snapshots = false;
        assert_eq!(keep(&workspace, "rm a.txt"), Kept::default());
        assert!(kept(&workspace).is_empty());
    }

    #[test]
    fn removing_thousands_of_small_files_writes_the_checkpoint_down_once() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join("build")).expect("build");
        for n in 0..MOST_FILES {
            std::fs::write(dir.path().join(format!("build/{n}.o")), "o").expect("file");
        }
        let workspace = workspace(dir.path());
        let journaled = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&journaled);
        let files = workspace.services.files.as_ref().expect("files");
        files.journaled(Arc::new(move |_: &crate::checkpoints::Kept| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }));
        let started = std::time::Instant::now();
        assert_eq!(keep(&workspace, "rm -rf build"), Kept { files: MOST_FILES, left_out: 0 });
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
        assert_eq!(journaled.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_note_is_given_only_when_something_was_left_out() {
        assert_eq!(Kept { files: 3, left_out: 0 }.note(), None);
        assert!(Kept { files: 3, left_out: 2 }.note().is_some_and(|note| note.contains("2 more")));
    }
}
