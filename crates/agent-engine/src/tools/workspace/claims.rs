use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use domain::claims::{Claim, Claims, warning};

use super::Workspace;

impl Workspace {
    pub fn claim(&self, path: &Path) -> String {
        if !self.policy.session_claims {
            return String::new();
        }
        let Some(registry) = registry(&self.cwd) else { return String::new() };
        let relative = path.strip_prefix(&self.cwd).unwrap_or(path).display().to_string();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs());
        let mine = Claim { pid: std::process::id(), session: self.session_id.clone(), path: relative, at_secs: now };
        match claimed(&registry, mine) {
            Ok(Some(other)) => warning(&other, now),
            Ok(None) => String::new(),
            Err(error) => {
                tracing::warn!(%error, "the session registry could not be read");
                String::new()
            }
        }
    }
}

fn registry(cwd: &Path) -> Option<PathBuf> {
    let root = cwd.canonicalize().ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    Some(
        std::env::temp_dir().join(concat!(brand::command!(), "-claims")).join(format!("{:016x}.json", hasher.finish())),
    )
}

fn claimed(registry: &Path, mine: Claim) -> std::io::Result<Option<Claim>> {
    let _held = platform::file_lock::Exclusive::on(registry)?;
    let mut claims: Claims =
        std::fs::read_to_string(registry).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    let other = claims.claim(mine, &platform::alive);
    let text = serde_json::to_string(&claims).map_err(std::io::Error::other)?;
    platform::private_file::replace(registry, text.as_bytes())?;
    Ok(other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_sessions_editing_one_file_are_told_about_each_other() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("lib.rs");
        let mut first = Workspace::new(dir.path());
        let mut second = Workspace::new(dir.path());
        first.policy.session_claims = true;
        second.policy.session_claims = true;

        assert_eq!(first.claim(&file), "");
        assert_eq!(first.claim(&file), "", "a session's own edits are no clash");
        let told = second.claim(&file);

        assert!(told.contains("<other-session>") && told.contains(&std::process::id().to_string()), "{told}");
        second.policy.session_claims = false;
        assert_eq!(second.claim(&file), "");
        if let Some(registry) = registry(dir.path()) {
            let _ = std::fs::remove_file(registry);
        }
    }
}
