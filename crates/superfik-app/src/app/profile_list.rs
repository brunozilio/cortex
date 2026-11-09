//! The browser profiles under the cache root, and lists of them kept in a
//! file beside them (the profiles still owed something when they next
//! open: logins brought over, cookies cleared), so the debt outlives the
//! run.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn profiles(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("profile-")))
        .collect()
}

pub struct ProfileList {
    path: PathBuf,
    dirs: HashSet<PathBuf>,
}

impl ProfileList {
    pub fn load(path: PathBuf) -> Self {
        let dirs =
            std::fs::read_to_string(&path).map(|text| text.lines().map(PathBuf::from).collect()).unwrap_or_default();
        Self { path, dirs }
    }

    #[cfg(test)]
    pub fn contains(&self, dir: &Path) -> bool {
        self.dirs.contains(dir)
    }

    pub fn iter(&self) -> impl Iterator<Item = &PathBuf> {
        self.dirs.iter()
    }

    pub fn remove(&mut self, dir: &Path) -> bool {
        let removed = self.dirs.remove(dir);
        if removed {
            self.save();
        }
        removed
    }

    pub fn extend(&mut self, dirs: impl IntoIterator<Item = PathBuf>) {
        self.dirs.extend(dirs);
        self.save();
    }

    pub fn clear(&mut self) {
        self.dirs.clear();
        self.save();
    }

    fn save(&self) {
        let result = if self.dirs.is_empty() {
            match std::fs::remove_file(&self.path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        } else {
            let lines: Vec<String> = self.dirs.iter().map(|dir| dir.display().to_string()).collect();
            std::fs::write(&self.path, lines.join("\n"))
        };
        if let Err(error) = result {
            tracing::warn!(%error, path = %self.path.display(), "could not keep the list of profiles");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_outlives_the_run_and_its_file_goes_once_empty() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("owed");
        let (a, b) = (root.path().join("profile-a"), root.path().join("profile-b"));
        let mut list = ProfileList::load(path.clone());
        list.extend([a.clone(), b.clone()]);
        let mut again = ProfileList::load(path.clone());
        assert!(again.contains(&a) && again.contains(&b));
        assert!(again.remove(&a));
        assert!(!again.remove(&a), "once");
        again.clear();
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn only_profile_folders_count() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::create_dir(root.path().join("profile-x"))?;
        std::fs::create_dir(root.path().join("Default"))?;
        std::fs::write(root.path().join("profile-file"), b"")?;
        assert_eq!(profiles(root.path()), vec![root.path().join("profile-x")]);
        Ok(())
    }
}
