use std::path::{Component, Path, PathBuf};

use super::Workspace;

fn settled(path: &Path) -> PathBuf {
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

impl Workspace {
    fn project_root(&self) -> PathBuf {
        let cwd = settled(&self.cwd);
        cwd.ancestors().find(|dir| dir.join(".git").exists()).map_or_else(|| cwd.clone(), Path::to_path_buf)
    }

    pub fn may_read(&self, path: &Path) -> bool {
        let path = settled(&if path.is_absolute() { path.to_path_buf() } else { self.cwd.join(path) });
        path.starts_with(self.project_root())
            || self.added_dirs.iter().any(|dir| path.starts_with(settled(dir)))
            || path.starts_with(settled(&super::persisted::results_dir(&self.session_id)))
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
