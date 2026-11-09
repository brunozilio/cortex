//! Clearing the cards' cookies (Settings › Browser). A page that is open
//! has them deleted by its engine and reloads. A profile the engine has not
//! loaded this run has its cookie files deleted on disk. One it loaded and
//! closed may still be held by the engine, so it is cleared when it opens
//! again, or at the next launch, before the engine starts (a list of
//! them is kept beside the profiles until then).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use superfik_browser::{BrowserId, Call, Profile};

use super::profile_list::{ProfileList, profiles};

const PENDING: &str = "clear-cookies";
const COOKIE_FILES: [&str; 2] = ["Cookies", "Cookies-journal"];

pub struct CookieClearing {
    root: PathBuf,
    loaded: HashSet<PathBuf>,
    pending: ProfileList,
}

impl CookieClearing {
    pub fn new(root: PathBuf) -> Self {
        let mut pending = ProfileList::load(root.join(PENDING));
        pending.iter().for_each(|dir| delete_cookies(dir));
        pending.clear();
        Self { root, loaded: HashSet::new(), pending }
    }

    pub fn opened(&mut self, id: BrowserId, profile: &Profile) -> Option<Call> {
        let dir = profile.dir(&self.root)?;
        self.loaded.insert(dir.clone());
        self.pending.remove(&dir).then_some(Call::ClearCookies(id))
    }

    pub fn clear(&mut self, open: &[(BrowserId, Profile)]) -> Vec<Call> {
        let open_dirs: HashSet<PathBuf> = open.iter().filter_map(|(_, p)| p.dir(&self.root)).collect();
        let mut held = Vec::new();
        for dir in profiles(&self.root) {
            if open_dirs.contains(&dir) {
                continue;
            }
            if self.loaded.contains(&dir) {
                held.push(dir);
            } else {
                delete_cookies(&dir);
            }
        }
        self.pending.extend(held);
        open.iter().flat_map(|(id, _)| [Call::ClearCookies(*id), Call::Reload(*id)]).collect()
    }
}

fn delete_cookies(profile: &Path) {
    for file in COOKIE_FILES {
        let _ = std::fs::remove_file(profile.join(file));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with_cookies(root: &Path, name: &str) -> std::io::Result<PathBuf> {
        let dir = root.join(format!("profile-{name}"));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("Cookies"), b"x")?;
        Ok(dir)
    }

    #[test]
    fn open_pages_clear_now_and_unloaded_profiles_lose_their_files() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let open = profile_with_cookies(root.path(), "open")?;
        let idle = profile_with_cookies(root.path(), "idle")?;
        let mut clearing = CookieClearing::new(root.path().to_owned());
        let id = BrowserId::new();
        assert!(clearing.opened(id, &Profile::Persistent("open".into())).is_none());
        let calls = clearing.clear(&[(id, Profile::Persistent("open".into()))]);
        assert!(matches!(calls[..], [Call::ClearCookies(a), Call::Reload(b)] if a == id && b == id));
        assert!(open.join("Cookies").exists(), "the engine clears an open page itself");
        assert!(!idle.join("Cookies").exists());
        Ok(())
    }

    #[test]
    fn a_closed_loaded_profile_clears_when_it_opens_or_at_the_next_launch() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = profile_with_cookies(root.path(), "a")?;
        let profile = Profile::Persistent("a".into());
        let mut clearing = CookieClearing::new(root.path().to_owned());
        clearing.opened(BrowserId::new(), &profile);
        clearing.clear(&[]);
        assert!(dir.join("Cookies").exists(), "the engine may still hold it");
        assert!(root.path().join(PENDING).exists());
        let id = BrowserId::new();
        assert!(matches!(clearing.opened(id, &profile), Some(Call::ClearCookies(c)) if c == id));
        assert!(!root.path().join(PENDING).exists(), "cleared live: nothing left for the next launch");
        clearing.clear(&[]);
        drop(clearing);
        CookieClearing::new(root.path().to_owned());
        assert!(!dir.join("Cookies").exists(), "cleared at the next launch");
        Ok(())
    }
}
