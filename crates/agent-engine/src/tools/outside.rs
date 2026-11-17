use std::path::{Component, Path, PathBuf};

use super::Workspace;

pub(crate) fn settled(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    let mut existing = out.as_path();
    let mut rest = Vec::new();
    while let Some(parent) = existing.parent() {
        if let Ok(real) = existing.canonicalize() {
            return rest.into_iter().rev().fold(real, |acc: PathBuf, name| acc.join(name));
        }
        rest.extend(existing.file_name());
        existing = parent;
    }
    out
}

const OUTSIDE_THE_PROJECT: &str = "This writes outside the project and the folders you added:";
const GIT_FOLDER: &str =
    "This writes inside the repository's .git folder, where hooks and configuration can run commands:";
const OWN_CONFIGURATION: &str =
    concat!("This changes ", brand::name!(), "'s own configuration, which can grant permissions and run code:");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedWrite {
    pub path: PathBuf,
    pub why: &'static str,
}

fn protected(inside: &Path) -> Option<&'static str> {
    inside.components().find_map(|part| {
        let name = part.as_os_str().to_string_lossy();
        if name.eq_ignore_ascii_case(".git") {
            Some(GIT_FOLDER)
        } else if name.eq_ignore_ascii_case(brand::DIR) {
            Some(OWN_CONFIGURATION)
        } else {
            None
        }
    })
}

impl Workspace {
    fn project_root(&self) -> PathBuf {
        domain::trust::project_root(&settled(&self.cwd), &|dir| dir.join(".git").exists())
    }

    fn settled_here(&self, path: &Path) -> PathBuf {
        settled(&if path.is_absolute() { path.to_path_buf() } else { self.cwd.join(path) })
    }

    fn root_holding(&self, settled_path: &Path) -> Option<PathBuf> {
        std::iter::once(self.project_root())
            .chain(self.added_dirs.iter().map(|dir| settled(dir)))
            .find(|root| settled_path.starts_with(root))
    }

    pub fn may_read(&self, path: &Path) -> bool {
        let path = self.settled_here(path);
        self.root_holding(&path).is_some()
            || path.starts_with(settled(&super::persisted::results_dir(&self.session_id)))
    }

    pub fn guarded_write(&self, path: &Path) -> Option<GuardedWrite> {
        let path = self.settled_here(path);
        let why = match self.root_holding(&path) {
            None => OUTSIDE_THE_PROJECT,
            Some(root) => path.strip_prefix(&root).ok().and_then(protected)?,
        };
        Some(GuardedWrite { path, why })
    }

    pub fn outside<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Option<PathBuf> {
        paths.into_iter().map(|path| self.resolve(path)).find(|path| !self.may_read(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_is_inside_and_the_rest_is_not() {
        let home = tempfile::tempdir().expect("dir");
        let project = home.path().join("rocket");
        std::fs::create_dir_all(project.join(".git")).expect("repo");
        std::fs::create_dir_all(project.join("src")).expect("src");
        std::fs::create_dir_all(home.path().join(".ssh")).expect("ssh");
        std::fs::write(home.path().join("notes.md"), "n").expect("notes");
        let workspace = Workspace::new(project.join("src"));
        assert!(workspace.may_read(Path::new("main.rs")), "relative, not there yet");
        assert!(workspace.may_read(&project.join("Cargo.toml")), "the repository's root");
        assert!(!workspace.may_read(&home.path().join(".ssh/id_ed25519")));
        assert!(!workspace.may_read(Path::new("../../.ssh/id_ed25519")), "no climbing out");
        assert!(!workspace.may_read(&home.path().join("notes.md")));
        let outside = workspace.outside(["main.rs", "/etc/hosts"]);
        #[cfg(unix)]
        assert_eq!(outside, Some(PathBuf::from("/etc/hosts")));
        #[cfg(windows)]
        assert!(outside.is_some_and(|path| path.ends_with("etc/hosts")));
    }

    #[test]
    fn this_session_s_saved_outputs_are_inside_but_another_session_s_are_not() {
        let project = tempfile::tempdir().expect("dir");
        let workspace = Workspace::new(project.path());
        let saved = super::super::persisted::results_dir(&workspace.session_id).join("call_1.txt");
        assert!(workspace.may_read(&saved));
        assert!(!workspace.may_read(&super::super::persisted::results_dir("another").join("call_1.txt")));
    }

    #[test]
    fn an_added_folder_is_inside_like_the_project() {
        let home = tempfile::tempdir().expect("dir");
        let (project, shared) = (home.path().join("rocket"), home.path().join("shared"));
        std::fs::create_dir_all(&project).expect("project");
        std::fs::create_dir_all(shared.join("lib")).expect("shared");
        let mut workspace = Workspace::new(&project);
        workspace.added_dirs = vec![shared.clone()];
        assert!(workspace.may_read(&shared.join("lib/a.rs")));
        assert!(!workspace.may_read(&home.path().join("other/a.rs")));
        assert!(!workspace.may_read(&shared.join("../other/a.rs")), "no climbing out of it either");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_project_is_outside() {
        let home = tempfile::tempdir().expect("dir");
        let project = home.path().join("rocket");
        std::fs::create_dir_all(&project).expect("project");
        std::fs::create_dir_all(home.path().join(".ssh")).expect("ssh");
        std::os::unix::fs::symlink(home.path().join(".ssh"), project.join("keys")).expect("link");
        let workspace = Workspace::new(&project);
        assert!(!workspace.may_read(Path::new("keys/id_ed25519")));
    }
}
