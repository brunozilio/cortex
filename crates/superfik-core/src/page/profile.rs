//! Where a card keeps its cookies, storage and cache.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Profile {
    Persistent(String),
    Ephemeral,
}

impl Profile {
    pub fn dir(&self, root: &Path) -> Option<PathBuf> {
        match self {
            Profile::Ephemeral => None,
            Profile::Persistent(name) => {
                let safe: String = name
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                    .collect();
                let safe = if safe.trim_matches('_').is_empty() { "default".to_owned() } else { safe };
                Some(root.join(format!("profile-{safe}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_profiles_live_under_the_root_and_cannot_escape_it() {
        let root = Path::new("/cache");
        assert_eq!(Profile::Persistent("abc-1".into()).dir(root), Some(PathBuf::from("/cache/profile-abc-1")));
        assert_eq!(Profile::Persistent("../../etc".into()).dir(root), Some(PathBuf::from("/cache/profile-______etc")));
        assert_eq!(Profile::Persistent("..".into()).dir(root), Some(PathBuf::from("/cache/profile-default")));
        assert_eq!(Profile::Ephemeral.dir(root), None);
    }
}
