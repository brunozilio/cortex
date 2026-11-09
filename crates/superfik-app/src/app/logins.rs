//! Logins brought over from the user's own browser (Settings › Browser).
//!
//! Importing reads every cookie that browser keeps (off the main thread:
//! macOS or a Linux keyring may prompt) and sets them in every open page's
//! profile; other existing profiles are owed them until their page next
//! opens (a list kept beside the profiles, so it outlives the run). What
//! was read is kept as a seed that new profiles start with. The browser is
//! read only when the user imports, so the Keychain never asks at launch.
//! Private pages never take them.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use superfik_browser::import::{BrowserKind, Cookie};
use superfik_browser::{BrowserId, Call, Profile};

use super::login_seed::{self, Seed};
use super::profile_list::{ProfileList, profiles};
use crate::state::Action;

const OWED: &str = "logins-owed";

struct Reading {
    kind: BrowserKind,
    answer: Receiver<Result<Vec<Cookie>, String>>,
}

pub struct Logins {
    root: PathBuf,
    from: Option<BrowserKind>,
    seed: Option<Seed>,
    reading: Option<Reading>,
    owed: ProfileList,
}

impl Logins {
    pub fn new(root: PathBuf) -> Self {
        let owed = ProfileList::load(root.join(OWED));
        let seed = login_seed::load(&root);
        Self { root, from: None, seed, reading: None, owed }
    }

    pub fn forget(&mut self) {
        self.owed.clear();
        self.seed = None;
        login_seed::remove(&self.root);
    }

    pub fn follow(&mut self, from: Option<BrowserKind>) {
        self.from = from;
    }

    pub fn import(&mut self, kind: BrowserKind) {
        let (reply, answer) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("superfik-logins".into()).spawn(move || {
            let _ = reply.send(read(kind));
        });
        match spawned {
            Ok(_) => self.reading = Some(Reading { kind, answer }),
            Err(error) => tracing::warn!(%error, "could not start reading logins"),
        }
    }

    pub fn opened(&mut self, id: BrowserId, profile: &Profile) -> Vec<Call> {
        let Some(kind) = self.from else { return Vec::new() };
        let Some(dir) = profile.dir(&self.root) else { return Vec::new() };
        if dir.exists() && !self.owed.remove(&dir) {
            return Vec::new();
        }
        match &self.seed {
            Some(seed) if seed.browser == kind => vec![Call::SetCookies { id, cookies: seed.cookies.clone() }],
            _ => Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn seed_for_test(&mut self, browser: BrowserKind) {
        let cookie = Cookie {
            name: "s".into(),
            value: "v".into(),
            domain: ".github.com".into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            same_site: None,
            expires: None,
        };
        self.seed = Some(Seed { browser, cookies: vec![cookie] });
    }

    pub fn is_busy(&self) -> bool {
        self.reading.is_some()
    }

    pub fn poll(&mut self, open: &[(BrowserId, Profile)]) -> (Vec<Call>, Option<Action>) {
        let Some(reading) = &self.reading else { return (Vec::new(), None) };
        let result = match reading.answer.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return (Vec::new(), None),
            Err(TryRecvError::Disconnected) => Err("The import stopped.".to_owned()),
        };
        let kind = reading.kind;
        self.reading = None;
        let cookies = match result {
            Ok(cookies) => cookies,
            Err(error) => {
                tracing::warn!(browser = kind.name(), %error, "logins not read");
                return (Vec::new(), Some(Action::LoginsImported(kind, Err(error))));
            }
        };
        let count = cookies.len();
        let mut calls = Vec::new();
        let mut signed: HashSet<PathBuf> = HashSet::new();
        for (id, profile) in open {
            let Some(dir) = profile.dir(&self.root) else { continue };
            calls.push(Call::SetCookies { id: *id, cookies: cookies.clone() });
            calls.push(Call::Reload(*id));
            signed.insert(dir);
        }
        self.owed.extend(profiles(&self.root).into_iter().filter(|dir| !signed.contains(dir)));
        let seed = Seed { browser: kind, cookies };
        login_seed::save(&self.root, &seed);
        self.seed = Some(seed);
        (calls, Some(Action::LoginsImported(kind, Ok(count))))
    }
}

fn read(kind: BrowserKind) -> Result<Vec<Cookie>, String> {
    let home = std::env::home_dir().ok_or("No home folder.")?;
    let sources = superfik_browser::import::sources(&home);
    let source = sources.iter().find(|s| s.kind == kind).ok_or_else(|| format!("{} is not installed.", kind.name()))?;
    read_source(source)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn read_source(source: &superfik_browser::import::Source) -> Result<Vec<Cookie>, String> {
    superfik_browser::import::read_all(source)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn read_source(source: &superfik_browser::import::Source) -> Result<Vec<Cookie>, String> {
    Err(format!("Bringing logins over from {} is not available on this system yet.", source.kind.name()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn cookie() -> Cookie {
        Cookie {
            name: "s".into(),
            value: "v".into(),
            domain: ".github.com".into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            same_site: None,
            expires: None,
        }
    }

    fn answered(logins: &mut Logins, kind: BrowserKind, result: Result<Vec<Cookie>, String>) {
        let (reply, answer) = std::sync::mpsc::channel();
        let _ = reply.send(result);
        logins.reading = Some(Reading { kind, answer });
    }

    fn imported(root: &Path, kind: BrowserKind) -> Logins {
        let mut logins = Logins::new(root.to_owned());
        logins.follow(Some(kind));
        answered(&mut logins, kind, Ok(vec![cookie()]));
        logins.poll(&[]);
        logins
    }

    #[test]
    fn only_new_persistent_profiles_take_logins() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let mut logins = imported(root.path(), BrowserKind::Dia);
        logins.follow(None);
        let fresh = Profile::Persistent("new".into());
        assert!(logins.opened(BrowserId::new(), &fresh).is_empty(), "no browser chosen");
        logins.follow(Some(BrowserKind::Dia));
        assert!(matches!(logins.opened(BrowserId::new(), &fresh)[..], [Call::SetCookies { .. }]));
        assert!(logins.opened(BrowserId::new(), &Profile::Ephemeral).is_empty(), "private pages stay clean");
        std::fs::create_dir(root.path().join("profile-old"))?;
        assert!(logins.opened(BrowserId::new(), &Profile::Persistent("old".into())).is_empty(), "already used");
        Ok(())
    }

    #[test]
    fn after_a_restart_new_profiles_start_signed_in_without_reading_the_browser_again() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        imported(root.path(), BrowserKind::Dia);
        let mut later = Logins::new(root.path().to_owned());
        later.follow(Some(BrowserKind::Dia));
        let calls = later.opened(BrowserId::new(), &Profile::Persistent("new".into()));
        assert!(matches!(&calls[..], [Call::SetCookies { cookies, .. }] if *cookies == vec![cookie()]));
        assert!(!later.is_busy(), "the Keychain is not asked");
        later.follow(Some(BrowserKind::Arc));
        assert!(later.opened(BrowserId::new(), &Profile::Persistent("other".into())).is_empty(), "another browser");
        assert!(!later.is_busy());
        Ok(())
    }

    #[test]
    fn asked_imports_reach_the_open_pages_but_not_private_ones_and_report() {
        let mut logins = Logins::new(PathBuf::from("/nowhere"));
        let (page, private) = (BrowserId::new(), BrowserId::new());
        let open = [(page, Profile::Persistent("a".into())), (private, Profile::Ephemeral)];
        answered(&mut logins, BrowserKind::Chrome, Ok(vec![cookie(), cookie()]));
        let (calls, report) = logins.poll(&open);
        assert!(matches!(calls[..], [Call::SetCookies { id, .. }, Call::Reload(r)] if id == page && r == page));
        assert!(matches!(report, Some(Action::LoginsImported(BrowserKind::Chrome, Ok(2)))));
        assert!(!logins.is_busy());
    }

    #[test]
    fn sessions_already_used_take_the_logins_when_they_next_open_even_after_a_restart() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::create_dir(root.path().join("profile-old"))?;
        imported(root.path(), BrowserKind::Dia);
        let mut later = Logins::new(root.path().to_owned());
        later.follow(Some(BrowserKind::Dia));
        let old = Profile::Persistent("old".into());
        assert!(matches!(later.opened(BrowserId::new(), &old)[..], [Call::SetCookies { .. }]));
        assert!(later.opened(BrowserId::new(), &old).is_empty(), "once");
        Ok(())
    }

    #[test]
    fn clearing_cookies_forgets_what_profiles_were_owed_and_the_seed() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::create_dir(root.path().join("profile-old"))?;
        let mut logins = imported(root.path(), BrowserKind::Dia);
        logins.forget();
        assert!(logins.opened(BrowserId::new(), &Profile::Persistent("old".into())).is_empty());
        assert!(logins.opened(BrowserId::new(), &Profile::Persistent("new".into())).is_empty());
        assert!(!login_seed::path(root.path()).exists());
        Ok(())
    }

    #[test]
    fn a_failed_import_reports_and_keeps_the_seed() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let mut logins = imported(root.path(), BrowserKind::Dia);
        answered(&mut logins, BrowserKind::Dia, Err("denied".into()));
        let (calls, report) = logins.poll(&[]);
        assert!(calls.is_empty());
        assert!(matches!(report, Some(Action::LoginsImported(BrowserKind::Dia, Err(_)))));
        assert!(!logins.opened(BrowserId::new(), &Profile::Persistent("new".into())).is_empty());
        Ok(())
    }
}
