use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use platform::keychain::Unreachable;
use platform::sync::Locked;

use super::*;

#[derive(Default)]
struct FakeKeychain {
    items: Mutex<HashMap<String, String>>,
    gone: AtomicBool,
    forgetful: bool,
    sets: AtomicUsize,
}

impl FakeKeychain {
    fn forgetful() -> Self {
        Self { forgetful: true, ..Self::default() }
    }

    fn gone() -> Self {
        let keychain = Self::default();
        keychain.goes_away();
        keychain
    }

    fn goes_away(&self) {
        self.gone.store(true, Ordering::SeqCst);
    }

    fn holds(&self, path: &Path) -> Option<String> {
        self.items.locked().get(&path.display().to_string()).cloned()
    }

    fn reachable(&self) -> Result<(), Unreachable> {
        if self.gone.load(Ordering::SeqCst) { Err(Unreachable("no keychain here".to_owned())) } else { Ok(()) }
    }
}

impl Keychain for FakeKeychain {
    fn get(&self, account: &str) -> Result<Option<String>, Unreachable> {
        self.reachable()?;
        Ok(self.items.locked().get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), Unreachable> {
        self.reachable()?;
        self.sets.fetch_add(1, Ordering::SeqCst);
        if !self.forgetful {
            self.items.locked().insert(account.to_owned(), secret.to_owned());
        }
        Ok(())
    }
}

fn on_disk(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read")
}

fn file_key(path: &Path) -> [u8; 32] {
    at(path, Keeper::File).expect("a key in a file")
}

#[test]
fn a_fresh_install_keeps_the_key_in_the_keychain_and_only_a_pointer_on_disk() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();

    let key = at(&path, Keeper::Keychain(&keychain)).expect("made");

    assert_eq!(keychain.holds(&path), Some(hex(&key)));
    assert_eq!(on_disk(&path), IN_KEYCHAIN, "no key material sits on disk");
    assert_eq!(at(&path, Keeper::Keychain(&keychain)).expect("again"), key, "made once, then read");
    assert_eq!(keychain.sets.load(Ordering::SeqCst), 1);
}

#[test]
fn a_key_file_moves_into_the_keychain_and_leaves_only_a_pointer() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let key = file_key(&path);
    let keychain = FakeKeychain::default();

    assert_eq!(at(&path, Keeper::Keychain(&keychain)).expect("moved"), key, "the same key seals the same file");

    assert_eq!(keychain.holds(&path), Some(hex(&key)));
    assert_eq!(on_disk(&path), IN_KEYCHAIN);
}

#[test]
fn a_keychain_that_does_not_give_the_key_back_leaves_the_key_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let key = file_key(&path);
    let keychain = FakeKeychain::forgetful();

    assert_eq!(at(&path, Keeper::Keychain(&keychain)).expect("still the file's"), key);

    assert_eq!(on_disk(&path), hex(&key), "the file is deleted only after a read-back");
}

#[test]
fn a_fresh_install_that_cannot_store_in_the_keychain_keeps_its_key_in_a_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::forgetful();

    let key = at(&path, Keeper::Keychain(&keychain)).expect("made");

    assert_eq!(on_disk(&path), hex(&key));
}

#[test]
fn a_fresh_install_with_no_keychain_keeps_its_key_in_a_file_and_moves_it_once_one_appears() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");

    let key = at(&path, Keeper::Keychain(&FakeKeychain::gone())).expect("made");
    assert_eq!(on_disk(&path), hex(&key), "headless machines still keep keys");

    let keychain = FakeKeychain::default();
    assert_eq!(at(&path, Keeper::Keychain(&keychain)).expect("moved"), key);
    assert_eq!(keychain.holds(&path), Some(hex(&key)));
}

#[test]
fn a_keychain_that_goes_away_is_an_error_that_offers_the_file_and_makes_no_new_key() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();
    let key = at(&path, Keeper::Keychain(&keychain)).expect("made");
    keychain.goes_away();

    let error = at(&path, Keeper::Keychain(&keychain)).expect_err("no new key orphans the vault");

    assert!(error.contains("cannot be reached") && error.contains("no keychain here"), "{error}");
    assert!(error.contains(&format!("{CHOICE}=file")), "the fallback is offered: {error}");
    assert_eq!(on_disk(&path), IN_KEYCHAIN);
    assert_eq!(keychain.items.locked().values().cloned().collect::<Vec<_>>(), [hex(&key)]);
    assert_eq!(keychain.sets.load(Ordering::SeqCst), 1);
}

#[test]
fn a_key_missing_from_the_keychain_is_an_error_not_a_new_key() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();
    at(&path, Keeper::Keychain(&keychain)).expect("made");
    keychain.items.locked().clear();

    let error = at(&path, Keeper::Keychain(&keychain)).expect_err("no new key");

    assert!(error.contains("not in the system keychain") && error.contains("secrets.key"), "{error}");
    assert!(keychain.items.locked().is_empty());
    assert_eq!(on_disk(&path), IN_KEYCHAIN);
}

#[test]
fn a_key_already_in_the_keychain_is_used_when_no_file_points_to_it() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();
    let key = at(&path, Keeper::Keychain(&keychain)).expect("made");
    std::fs::remove_file(&path).expect("pointer lost");

    assert_eq!(at(&path, Keeper::Keychain(&keychain)).expect("found"), key);
    assert_eq!(on_disk(&path), IN_KEYCHAIN);
}

#[test]
fn the_file_backend_when_chosen_never_stores_in_the_keychain() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();

    let key = at(&path, Keeper::chosen(Some("file"), &keychain)).expect("made");

    assert_eq!(on_disk(&path), hex(&key));
    assert_eq!(keychain.sets.load(Ordering::SeqCst), 0);
    assert_eq!(at(&path, Keeper::chosen(Some(" FILE "), &keychain)).expect("again"), key);
}

#[test]
fn choosing_the_file_takes_the_key_back_from_a_keychain_that_answers() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();
    let key = at(&path, Keeper::Keychain(&keychain)).expect("made");

    assert_eq!(at(&path, Keeper::chosen(Some("file"), &keychain)).expect("taken back"), key);

    assert_eq!(on_disk(&path), hex(&key));
}

#[test]
fn the_file_default_never_replaces_a_key_the_keychain_holds_when_it_cannot_take_it_back() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();
    let old = at(&path, Keeper::Keychain(&keychain)).expect("made");
    keychain.goes_away();

    let refused = at(&path, Keeper::chosen(None, &keychain)).expect_err("no new key over sealed entries");

    assert!(refused.contains("nothing was replaced"), "{refused}");
    assert_eq!(on_disk(&path), IN_KEYCHAIN);
    keychain.gone.store(false, Ordering::SeqCst);
    assert_eq!(at(&path, Keeper::chosen(None, &keychain)).expect("taken back once it answers"), old);
    assert_eq!(on_disk(&path), hex(&old));
}

#[test]
fn the_file_default_never_replaces_a_key_the_keychain_has_lost() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    std::fs::write(&path, IN_KEYCHAIN).expect("pointer");
    let keychain = FakeKeychain::default();

    let refused = at(&path, Keeper::chosen(None, &keychain)).expect_err("no new key over sealed entries");

    assert!(refused.contains("no longer holds it"), "{refused}");
    assert_eq!(on_disk(&path), IN_KEYCHAIN);
}

#[test]
fn only_asking_for_the_keychain_uses_it_and_the_file_is_the_default() {
    let keychain = FakeKeychain::default();
    for choice in [Some("keychain"), Some(" KEYCHAIN ")] {
        assert!(matches!(Keeper::chosen(choice, &keychain), Keeper::Keychain(_)), "{choice:?}");
    }
    for choice in [None, Some(""), Some("file"), Some("keychains")] {
        assert!(matches!(Keeper::chosen(choice, &keychain), Keeper::FileInsteadOf(_)), "{choice:?}");
    }
    assert_eq!(CHOICE, format!("{}_SECRETS", brand::ENV));
}

#[test]
fn by_default_a_fresh_install_keeps_its_key_in_a_file_and_never_touches_the_keychain() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();

    let key = at(&path, Keeper::chosen(None, &keychain)).expect("made");

    assert_eq!(on_disk(&path), hex(&key));
    assert_eq!(keychain.sets.load(Ordering::SeqCst), 0);
    assert_eq!(at(&path, Keeper::chosen(None, &keychain)).expect("again"), key);
}

#[test]
fn first_runs_at_once_with_a_keychain_end_with_one_key() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let keychain = FakeKeychain::default();
    let start = std::sync::Barrier::new(16);
    let made: Vec<[u8; 32]> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    at(&path, Keeper::Keychain(&keychain)).expect("key")
                })
            })
            .collect();
        runs.into_iter().map(|run| run.join().expect("join")).collect()
    });
    assert!(made.iter().all(|key| keychain.holds(&path) == Some(hex(key))), "every run seals with the one key kept");
    assert_eq!(keychain.sets.load(Ordering::SeqCst), 1);
}

#[cfg(unix)]
#[test]
fn a_key_that_cannot_be_read_is_an_error_and_is_left_as_it_was() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let key = file_key(&path);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("lock out");
    if std::fs::read(&path).is_ok() {
        return;
    }

    let error = at(&path, Keeper::File).expect_err("an unreadable key is not replaced");

    assert!(error.contains("secrets.key"), "{error}");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("let in");
    assert_eq!(on_disk(&path), hex(&key), "the vault's key is still the one made");
}

#[test]
fn a_folder_where_the_key_should_be_is_an_error_not_a_new_key() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    std::fs::create_dir(&path).expect("folder");
    assert!(at(&path, Keeper::File).is_err());
    assert!(at(&path, Keeper::Keychain(&FakeKeychain::default())).is_err());
    assert!(path.is_dir());
}

#[test]
fn first_runs_at_once_end_with_one_key() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("secrets.key");
    let start = std::sync::Barrier::new(16);
    let made: Vec<[u8; 32]> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    file_key(&path)
                })
            })
            .collect();
        runs.into_iter().map(|run| run.join().expect("join")).collect()
    });
    let kept = file_key(&path);
    assert!(made.iter().all(|key| *key == kept), "every first run seals with the key that was kept");
}
