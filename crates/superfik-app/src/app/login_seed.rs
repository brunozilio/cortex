//! What the last import of logins read, kept beside the profiles so new
//! profiles start signed in without asking the browser (and the Keychain)
//! again. Readable by the user only: these are sign-ins.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use superfik_browser::import::{BrowserKind, Cookie};

const FILE: &str = "logins-seed.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Seed {
    pub browser: BrowserKind,
    pub cookies: Vec<Cookie>,
}

pub fn path(root: &Path) -> PathBuf {
    root.join(FILE)
}

pub fn load(root: &Path) -> Option<Seed> {
    let text = std::fs::read_to_string(path(root)).ok()?;
    serde_json::from_str(&text).map_err(|error| tracing::warn!(%error, "imported logins unreadable")).ok()
}

pub fn save(root: &Path, seed: &Seed) {
    let result = serde_json::to_vec(seed)
        .map_err(std::io::Error::other)
        .and_then(|bytes| superfik_private_file::write(&path(root), &bytes));
    if let Err(error) = result {
        tracing::warn!(%error, "could not keep the imported logins");
    }
}

pub fn remove(root: &Path) {
    match std::fs::remove_file(path(root)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            tracing::warn!(%error, "could not forget the imported logins");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_outlives_the_run_until_forgotten() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        save(root.path(), &Seed { browser: BrowserKind::Dia, cookies: Vec::new() });
        assert!(matches!(load(root.path()), Some(Seed { browser: BrowserKind::Dia, .. })));
        remove(root.path());
        assert!(load(root.path()).is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_seed_is_readable_by_the_user_only() -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir()?;
        save(root.path(), &Seed { browser: BrowserKind::Dia, cookies: Vec::new() });
        assert_eq!(std::fs::metadata(path(root.path()))?.permissions().mode() & 0o777, 0o600);
        Ok(())
    }
}
