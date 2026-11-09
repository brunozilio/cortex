//! Bringing the user's logins over from their own browser: the cookies
//! Chrome, Dia, Arc, Brave, Edge, Chromium or Vivaldi keep, copied into the
//! cards' profiles so the cards are signed in too.
//!
//! Only when the user asks (Settings › Browser); macOS asks them before
//! handing over the browser's key, Linux's keyring may ask to be unlocked.
//! [`chromium`] is the pure part (finding and decrypting); [`read_all`] is
//! the IO.

#[cfg(not(target_arch = "wasm32"))]
pub mod chromium;
#[cfg(target_os = "macos")]
pub mod icon;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    Chrome,
    Dia,
    Arc,
    Brave,
    Edge,
    Chromium,
    Vivaldi,
}

impl BrowserKind {
    pub const ALL: [BrowserKind; 7] = [
        BrowserKind::Chrome,
        BrowserKind::Dia,
        BrowserKind::Arc,
        BrowserKind::Brave,
        BrowserKind::Edge,
        BrowserKind::Chromium,
        BrowserKind::Vivaldi,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BrowserKind::Chrome => "Chrome",
            BrowserKind::Dia => "Dia",
            BrowserKind::Arc => "Arc",
            BrowserKind::Brave => "Brave",
            BrowserKind::Edge => "Edge",
            BrowserKind::Chromium => "Chromium",
            BrowserKind::Vivaldi => "Vivaldi",
        }
    }

    fn places(self, system: System) -> (&'static [&'static str], &'static str) {
        match (system, self) {
            (System::Mac, BrowserKind::Chrome) => {
                (&["Library/Application Support/Google/Chrome"], "Chrome Safe Storage")
            }
            (System::Mac, BrowserKind::Dia) => (&["Library/Application Support/Dia/User Data"], "Dia Safe Storage"),
            (System::Mac, BrowserKind::Arc) => (&["Library/Application Support/Arc/User Data"], "Arc Safe Storage"),
            (System::Mac, BrowserKind::Brave) => {
                (&["Library/Application Support/BraveSoftware/Brave-Browser"], "Brave Safe Storage")
            }
            (System::Mac, BrowserKind::Edge) => {
                (&["Library/Application Support/Microsoft Edge"], "Microsoft Edge Safe Storage")
            }
            (System::Mac, BrowserKind::Chromium) => {
                (&["Library/Application Support/Chromium"], "Chromium Safe Storage")
            }
            (System::Mac, BrowserKind::Vivaldi) => (&["Library/Application Support/Vivaldi"], "Vivaldi Safe Storage"),
            (System::Linux, BrowserKind::Chrome) => (&[".config/google-chrome"], "chrome"),
            (System::Linux, BrowserKind::Brave) => (&[".config/BraveSoftware/Brave-Browser"], "brave"),
            (System::Linux, BrowserKind::Edge) => (&[".config/microsoft-edge"], "microsoft-edge"),
            (System::Linux, BrowserKind::Chromium) => {
                (&[".config/chromium", "snap/chromium/common/chromium"], "chromium")
            }
            (System::Linux, BrowserKind::Vivaldi) => (&[".config/vivaldi"], "vivaldi"),
            (System::Linux, BrowserKind::Dia | BrowserKind::Arc) => (&[], ""),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum System {
    Mac,
    Linux,
}

impl System {
    const THIS: Option<System> = if cfg!(target_os = "macos") {
        Some(System::Mac)
    } else if cfg!(target_os = "linux") {
        Some(System::Linux)
    } else {
        None
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSite>,
    pub expires: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl Cookie {
    pub fn url(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        format!("{scheme}://{}{}", self.domain.trim_start_matches('.'), self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: BrowserKind,
    pub dir: PathBuf,
    pub keychain: &'static str,
}

pub fn sources(home: &Path) -> Vec<Source> {
    System::THIS.map(|system| sources_on(home, system)).unwrap_or_default()
}

fn sources_on(home: &Path, system: System) -> Vec<Source> {
    BrowserKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let (dirs, keychain) = kind.places(system);
            let dir = dirs
                .iter()
                .map(|dir| home.join(dir))
                .find(|dir| dir.join("Local State").exists() || dir.join("Default").is_dir())?;
            Some(Source { kind, dir, keychain })
        })
        .collect()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn read_all(source: &Source) -> Result<Vec<Cookie>, String> {
    let name = source.kind.name();
    let db = chromium::cookie_db(&source.dir).ok_or_else(|| format!("{name} has no cookies here."))?;
    let keys = keys(source)?;
    let copy = std::env::temp_dir().join(format!("superfik-cookies-{}.sqlite", std::process::id()));
    std::fs::copy(&db, &copy).map_err(|error| format!("Could not read {name}'s cookies: {error}"))?;
    let rows = chromium::rows(&copy);
    let _ = std::fs::remove_file(&copy);
    let (rows, hashed) = rows.map_err(|error| format!("Could not read {name}'s cookies: {error}"))?;
    let cookies: Vec<Cookie> = rows.iter().filter_map(|row| row.cookie(&keys, hashed)).collect();
    if cookies.is_empty() && !rows.is_empty() {
        return Err(format!("Could not decrypt {name}'s cookies (is its keyring unlocked?)."));
    }
    Ok(cookies)
}

#[cfg(target_os = "macos")]
fn keys(source: &Source) -> Result<chromium::Keys, String> {
    let output = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-w", "-s", source.keychain])
        .output()
        .map_err(|error| format!("Could not ask the Keychain: {error}"))?;
    if !output.status.success() {
        return Err("macOS did not give the browser's key (was it allowed?).".to_owned());
    }
    Ok(chromium::Keys::mac(String::from_utf8_lossy(&output.stdout).trim()))
}

#[cfg(target_os = "linux")]
fn keys(source: &Source) -> Result<chromium::Keys, String> {
    let secret = match secret_service(source.keychain) {
        Ok(secret) => secret,
        Err(error) => {
            tracing::info!(browser = source.kind.name(), %error, "no Secret Service key; v10 cookies only");
            None
        }
    };
    Ok(chromium::Keys::linux(secret.as_deref()))
}

#[cfg(target_os = "linux")]
fn secret_service(application: &str) -> Result<Option<String>, String> {
    use dbus_secret_service::{EncryptionType, SecretService};
    let service = SecretService::connect(EncryptionType::Dh).map_err(|error| error.to_string())?;
    let found = service
        .search_items(std::collections::HashMap::from([("application", application)]))
        .map_err(|error| error.to_string())?;
    let Some(item) = found.unlocked.first().or(found.locked.first()) else { return Ok(None) };
    item.ensure_unlocked().map_err(|error| error.to_string())?;
    let secret = item.get_secret().map_err(|error| error.to_string())?;
    Ok(Some(String::from_utf8_lossy(&secret).into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_are_set_for_their_own_scheme_and_host() {
        let cookie = Cookie {
            name: "s".into(),
            value: "v".into(),
            domain: ".github.com".into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            same_site: Some(SameSite::Lax),
            expires: None,
        };
        assert_eq!(cookie.url(), "https://github.com/");
    }

    #[test]
    fn only_installed_browsers_are_offered() -> std::io::Result<()> {
        let home = tempfile::tempdir()?;
        std::fs::create_dir_all(home.path().join("Library/Application Support/Dia/User Data/Default"))?;
        let found = sources_on(home.path(), System::Mac);
        assert_eq!(found.iter().map(|s| s.kind).collect::<Vec<_>>(), [BrowserKind::Dia]);
        assert_eq!(found[0].keychain, "Dia Safe Storage");
        Ok(())
    }

    #[test]
    fn linux_browsers_live_under_the_config_folder_or_a_snap() -> std::io::Result<()> {
        let home = tempfile::tempdir()?;
        std::fs::create_dir_all(home.path().join(".config/google-chrome/Default"))?;
        std::fs::create_dir_all(home.path().join("snap/chromium/common/chromium/Default"))?;
        let found = sources_on(home.path(), System::Linux);
        assert_eq!(found.iter().map(|s| s.kind).collect::<Vec<_>>(), [BrowserKind::Chrome, BrowserKind::Chromium]);
        assert_eq!((found[0].keychain, found[1].keychain), ("chrome", "chromium"));
        assert_eq!(found[1].dir, home.path().join("snap/chromium/common/chromium"));
        Ok(())
    }
}
