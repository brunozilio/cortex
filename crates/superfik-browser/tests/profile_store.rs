use superfik_browser::profiles::ProfileStore;
use superfik_core::browser_profiles::{BrowserProfile, ProfileBook};

#[test]
fn library_size_limit_is_enforced_before_replacing_the_saved_file() {
    let root = tempfile::tempdir().expect("root");
    let store = ProfileStore::new(root.path().into());
    store.save(&ProfileBook::default()).expect("initial library");
    let original = std::fs::read(store.path()).expect("JSON");
    let mut book = ProfileBook::default();
    for index in 0..64 {
        let mut profile = BrowserProfile::new(format!("Large {index}"));
        profile.settings.fonts = (0..128).map(|n| format!("{n:04}{}", "a".repeat(124))).collect();
        book.profiles.push(profile);
    }
    assert!(store.save(&book).is_err());
    assert_eq!(std::fs::read(store.path()).expect("preserved"), original);
}

#[test]
fn a_new_authenticated_proxy_requires_a_saved_or_supplied_password() {
    use superfik_browser::profiles::CredentialStore;
    use superfik_core::browser_profiles::{ProfileId, Proxy, ProxyScheme};
    struct Empty;
    impl CredentialStore for Empty {
        fn read(&self, _: ProfileId) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn write(&self, _: ProfileId, _: Option<&str>) -> Result<(), String> {
            panic!("no credential write expected")
        }
    }
    let root = tempfile::tempdir().expect("root");
    let store = ProfileStore::new(root.path().into());
    let mut profile = BrowserProfile::new("Authenticated".into());
    profile.settings.proxy =
        Some(Proxy { scheme: ProxyScheme::Http, host: "127.0.0.1".into(), port: 8080, username: Some("lab".into()) });
    let mut book = ProfileBook::default();
    book.save(profile).expect("profile");
    assert!(store.save_with_credentials(&book, None, &Empty).is_err());
    assert!(!store.path().exists());
}

#[test]
fn proxy_password_is_kept_outside_profile_json_and_is_redacted_in_debug() {
    use std::sync::Mutex;
    use superfik_browser::profiles::{CredentialStore, SecretChange};
    use superfik_core::browser_profiles::{ProfileId, Proxy, ProxyScheme};
    #[derive(Default)]
    struct Secrets(Mutex<Option<String>>);
    impl CredentialStore for Secrets {
        fn read(&self, _: ProfileId) -> Result<Option<String>, String> {
            Ok(self.0.lock().expect("lock").clone())
        }
        fn write(&self, _: ProfileId, value: Option<&str>) -> Result<(), String> {
            *self.0.lock().expect("lock") = value.map(str::to_owned);
            Ok(())
        }
    }
    let root = tempfile::tempdir().expect("root");
    let store = ProfileStore::new(root.path().into());
    let mut profile = BrowserProfile::new("Proxy".into());
    profile.settings.proxy =
        Some(Proxy { scheme: ProxyScheme::Http, host: "127.0.0.1".into(), port: 8080, username: Some("lab".into()) });
    let change = SecretChange { id: profile.id, password: Some("lab-password-do-not-log".into()) };
    assert!(!format!("{change:?}").contains("lab-password-do-not-log"));
    let mut book = ProfileBook::default();
    book.save(profile).expect("profile");
    let secrets = Secrets::default();
    store.save_with_credentials(&book, Some(&change), &secrets).expect("save");
    assert_eq!(secrets.read(change.id).expect("read").as_deref(), Some("lab-password-do-not-log"));
    assert!(!std::fs::read_to_string(store.path()).expect("JSON").contains("lab-password-do-not-log"));
}

#[test]
fn saving_and_restoring_profiles_does_not_create_a_cef_conversation_directory() {
    let root = tempfile::tempdir().expect("root");
    let store = ProfileStore::new(root.path().to_path_buf());
    let mut book = ProfileBook::default();
    let profile = BrowserProfile::new("Research".into());
    let id = profile.id;
    book.save(profile).expect("profile");
    book.assign("conversation-a".into(), Some(id)).expect("assign");
    store.save(&book).expect("save");
    assert_eq!(store.load().expect("restore"), book);
    assert!(!root.path().join("profile-conversation-a").exists());
    let original = std::fs::read(store.path()).expect("JSON");
    book.schema = 999;
    assert!(store.save(&book).is_err());
    assert_eq!(std::fs::read(store.path()).expect("preserved"), original);
}
