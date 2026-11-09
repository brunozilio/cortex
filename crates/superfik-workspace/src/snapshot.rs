//! Describes a directory as a workspace snapshot.

use std::path::Path;

use superfik_core::WorkspaceSnapshot;

use crate::error::WorkspaceError;
use crate::git::GitInspector;

pub fn snapshot(git: &dyn GitInspector, path: &Path) -> Result<WorkspaceSnapshot, WorkspaceError> {
    std::fs::read_dir(path).map_err(|source| WorkspaceError::Io { path: path.to_path_buf(), source })?;
    let repository =
        git.inspect(path).inspect_err(|error| tracing::debug!(%error, path = %path.display(), "no repository")).ok();
    Ok(WorkspaceSnapshot { repository })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::Repository;

    use super::*;

    struct NoRepo;
    impl GitInspector for NoRepo {
        fn inspect(&self, root: &Path) -> Result<Repository, WorkspaceError> {
            Err(WorkspaceError::NotARepository(root.to_path_buf()))
        }
    }

    struct Repo;
    impl GitInspector for Repo {
        fn inspect(&self, root: &Path) -> Result<Repository, WorkspaceError> {
            Ok(Repository {
                root: root.to_path_buf(),
                slug: "a/b".into(),
                branch: "main".into(),
                ..Repository::default()
            })
        }
    }

    #[test]
    fn describes_the_repository() {
        let dir = tempfile::tempdir().expect("tempdir");
        let snap = snapshot(&Repo, dir.path()).expect("snapshot");
        assert_eq!(snap.repository.map(|r| r.root), Some(PathBuf::from(dir.path())));
    }

    #[test]
    fn missing_repository_is_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let snap = snapshot(&NoRepo, dir.path()).expect("snapshot");
        assert!(snap.repository.is_none());
    }

    #[test]
    fn unreadable_directory_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(snapshot(&Repo, &dir.path().join("missing")).is_err());
    }
}
