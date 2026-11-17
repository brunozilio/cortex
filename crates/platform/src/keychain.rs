#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Unreachable(pub String);

pub trait Keychain: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, Unreachable>;
    fn set(&self, account: &str, secret: &str) -> Result<(), Unreachable>;
}

#[derive(Debug, Clone)]
pub struct SystemKeychain {
    service: String,
}

impl SystemKeychain {
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
impl Keychain for SystemKeychain {
    fn get(&self, account: &str) -> Result<Option<String>, Unreachable> {
        let entry = keyring::Entry::new(&self.service, account).map_err(unreachable)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(unreachable(error)),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), Unreachable> {
        let entry = keyring::Entry::new(&self.service, account).map_err(unreachable)?;
        entry.set_password(secret).map_err(unreachable)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
impl Keychain for SystemKeychain {
    fn get(&self, _: &str) -> Result<Option<String>, Unreachable> {
        Err(Unreachable(NONE_HERE.to_owned()))
    }

    fn set(&self, _: &str, _: &str) -> Result<(), Unreachable> {
        Err(Unreachable(NONE_HERE.to_owned()))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
const NONE_HERE: &str = "this system has no keychain the app can use";

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn unreachable(error: keyring::Error) -> Unreachable {
    Unreachable(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreachable_keychain_says_what_the_system_said() {
        assert_eq!(Unreachable("no D-Bus session".to_owned()).to_string(), "no D-Bus session");
    }

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    #[test]
    #[ignore = "touches_the_real_keychain_of_the_user_running_it"]
    fn the_system_keychain_keeps_what_is_set_and_reads_it_back() {
        let keychain = SystemKeychain::new("keychain-test");
        let account = format!("probe-{}", std::process::id());
        assert_eq!(keychain.get(&account).expect("reachable"), None);
        keychain.set(&account, "kept").expect("set");
        assert_eq!(keychain.get(&account).expect("reachable").as_deref(), Some("kept"));
        keyring::Entry::new("keychain-test", &account).expect("entry").delete_credential().expect("cleaned up");
    }
}
