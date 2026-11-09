//! How a hosted server is set up: read from the environment once, at
//! start. Pure, so every rule is tested without a process.

use std::net::SocketAddr;
use std::path::PathBuf;

pub const DEFAULT_LISTEN: &str = "0.0.0.0:7420";
pub const DEFAULT_DATA: &str = "/var/lib/superfik";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub listen: SocketAddr,
    pub data: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("SUPERFIK_LISTEN is not an address: {0}")]
    Listen(String),
    #[error("cannot use the data folder {path}: {reason}; set SUPERFIK_DATA to a folder this user can write")]
    Data { path: String, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Server,
    Mac,
}

impl Host {
    pub fn this() -> Self {
        if cfg!(target_os = "macos") { Self::Mac } else { Self::Server }
    }
}

const MAC_LISTEN: &str = "127.0.0.1:7420";

impl Config {
    pub fn from(var: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        Self::on(var, Host::Server)
    }

    pub fn on(var: impl Fn(&str) -> Option<String>, host: Host) -> Result<Self, ConfigError> {
        let home = var("HOME").filter(|_| host == Host::Mac);
        let listen = var("SUPERFIK_LISTEN")
            .unwrap_or_else(|| if host == Host::Mac { MAC_LISTEN } else { DEFAULT_LISTEN }.to_owned());
        let listen = listen.parse().map_err(|_| ConfigError::Listen(listen))?;
        let data = var("SUPERFIK_DATA").map(PathBuf::from).unwrap_or_else(|| match home {
            Some(home) => PathBuf::from(home).join("Library/Application Support/Superfik/server"),
            None => PathBuf::from(DEFAULT_DATA),
        });
        Ok(Self { listen, data })
    }

    pub fn prepare(&self) -> Result<(), ConfigError> {
        let failed = |error: std::io::Error| ConfigError::Data {
            path: self.data.display().to_string(),
            reason: error.to_string(),
        };
        std::fs::create_dir_all(&self.data).map_err(failed)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.data, std::fs::Permissions::from_mode(0o700)).map_err(failed)?;
        }
        Ok(())
    }

    pub fn accounts_path(&self) -> PathBuf {
        self.data.join("accounts.json")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data.join("server.json")
    }
}

pub fn native_worker(var: Option<String>) -> Result<Option<PathBuf>, String> {
    let Some(worker) = var.filter(|worker| !worker.trim().is_empty()).map(PathBuf::from) else { return Ok(None) };
    if worker.is_file() {
        Ok(Some(worker))
    } else {
        Err(format!("SUPERFIK_NATIVE_WORKER is {}, which is not a file", worker.display()))
    }
}

pub fn secret_key(hex: &str) -> Option<[u8; 32]> {
    parse_key(hex.trim())
}

fn parse_key(hex: &str) -> Option<[u8; 32]> {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|at| hex.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
        .collect::<Option<_>>()?;
    bytes.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| pairs.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_owned())
    }

    #[test]
    fn defaults_listen_on_every_interface_and_keep_data_in_var_lib() {
        let config = Config::from(env(&[])).expect("config");
        assert_eq!(config.listen, "0.0.0.0:7420".parse().expect("addr"));
        assert_eq!(config.accounts_path(), PathBuf::from("/var/lib/superfik/accounts.json"));
    }

    #[test]
    fn a_data_folder_that_cannot_be_made_names_itself_and_the_variable() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("taken");
        std::fs::write(&file, "").expect("file");
        let data = file.join("superfik");
        let config = Config { listen: DEFAULT_LISTEN.parse().expect("addr"), data: data.clone() };
        let said = config.prepare().expect_err("not a folder").to_string();
        assert!(said.contains(&data.display().to_string()) && said.contains("SUPERFIK_DATA"), "{said}");
        let config = Config { data: dir.path().join("fresh"), ..config };
        assert_eq!(config.prepare(), Ok(()));
        assert!(config.data.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn the_data_folder_is_the_servers_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let config = Config { listen: DEFAULT_LISTEN.parse().expect("addr"), data: dir.path().join("data") };
        let mode = |config: &Config| std::fs::metadata(&config.data).expect("meta").permissions().mode() & 0o777;
        config.prepare().expect("made");
        assert_eq!(mode(&config), 0o700);
        std::fs::set_permissions(&config.data, std::fs::Permissions::from_mode(0o755)).expect("open it up");
        config.prepare().expect("again");
        assert_eq!(mode(&config), 0o700, "an existing folder is closed too");
    }

    #[test]
    fn on_a_mac_it_keeps_data_in_application_support_and_listens_on_this_computer_only() {
        let config = Config::on(env(&[("HOME", "/Users/olga")]), Host::Mac).expect("config");
        assert_eq!(config.data, PathBuf::from("/Users/olga/Library/Application Support/Superfik/server"));
        assert_eq!(config.listen, "127.0.0.1:7420".parse().expect("addr"));
        let set = Config::on(env(&[("HOME", "/Users/olga"), ("SUPERFIK_DATA", "/srv/s")]), Host::Mac).expect("set");
        assert_eq!(set.data, PathBuf::from("/srv/s"));
        assert_eq!(Config::on(env(&[("HOME", "/root")]), Host::Server), Config::from(env(&[])));
    }

    #[test]
    fn a_bad_address_is_named() {
        assert_eq!(Config::from(env(&[("SUPERFIK_LISTEN", "here")])), Err(ConfigError::Listen("here".into())));
    }

    #[test]
    fn a_worker_that_is_not_there_stops_the_start() {
        let dir = tempfile::tempdir().expect("dir");
        let worker = dir.path().join("native-as-runner");
        assert_eq!(native_worker(None), Ok(None));
        assert_eq!(native_worker(Some(" ".into())), Ok(None));
        let missing = native_worker(Some(worker.display().to_string())).expect_err("missing");
        assert!(missing.contains("SUPERFIK_NATIVE_WORKER"), "{missing}");
        std::fs::write(&worker, "#!/bin/sh\n").expect("write");
        assert_eq!(native_worker(Some(worker.display().to_string())), Ok(Some(worker)));
    }

    #[test]
    fn keys_are_64_hex_digits() {
        assert_eq!(parse_key(&"ab".repeat(32)), Some([0xab; 32]));
        assert_eq!(parse_key("abc"), None);
        assert_eq!(parse_key(&"zz".repeat(32)), None);
    }
}
