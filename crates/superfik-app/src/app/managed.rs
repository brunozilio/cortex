//! Reads what device management set for this app. On macOS that is a
//! configuration profile's payload for the app's bundle, which the system
//! writes to `/Library/Managed Preferences` (per user first). Elsewhere, a
//! JSON file IT drops in place. Key: `RequireOrganization` (boolean): no
//! prompt goes out until the app is connected to the team's server
//! (Settings › Server).

use std::path::{Path, PathBuf};

use crate::state::managed::Managed;

const BUNDLE: &str = "com.brunozilio.superfik";

fn places() -> Vec<PathBuf> {
    let mut places = Vec::new();
    if cfg!(target_os = "macos") {
        let root = Path::new("/Library/Managed Preferences");
        if let Ok(user) = std::env::var("USER") {
            places.push(root.join(user).join(format!("{BUNDLE}.plist")));
        }
        places.push(root.join(format!("{BUNDLE}.plist")));
    } else if cfg!(windows) {
        let data = std::env::var_os("ProgramData").map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from);
        places.push(data.join("Superfik").join("managed.json"));
    } else {
        places.push(PathBuf::from("/etc/superfik/managed.json"));
    }
    places
}

pub fn load() -> Managed {
    places().iter().find_map(|path| read(path)).unwrap_or_default()
}

fn read(path: &Path) -> Option<Managed> {
    if path.extension().is_some_and(|extension| extension == "json") {
        return from_json(&serde_json::from_slice(&std::fs::read(path).ok()?).ok()?);
    }
    from_plist(plist::Value::from_file(path).ok()?.as_dictionary()?)
}

fn from_plist(dict: &plist::Dictionary) -> Option<Managed> {
    Some(Managed {
        require_organization: dict.get("RequireOrganization").and_then(plist::Value::as_boolean).unwrap_or(false),
    })
}

fn from_json(json: &serde_json::Value) -> Option<Managed> {
    Some(Managed {
        require_organization: json.get("RequireOrganization").and_then(serde_json::Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_profile_payload_is_read() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join(format!("{BUNDLE}.plist"));
        let payload = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>OrganizationServer</key><string>superfik.acme.io</string>
  <key>RequireOrganization</key><true/>
</dict></plist>"#;
        std::fs::write(&path, payload).expect("write");
        assert_eq!(read(&path), Some(Managed { require_organization: true }));
    }

    #[test]
    fn a_json_file_is_read_and_nothing_is_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("managed.json");
        std::fs::write(&path, r#"{"OrganizationServer":"superfik.acme.io"}"#).expect("write");
        assert_eq!(read(&path), Some(Managed { require_organization: false }));
        assert_eq!(read(&dir.path().join("absent.json")), None);
    }
}
