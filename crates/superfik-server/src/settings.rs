//! What a hosted server keeps about itself in `<data>/server.json`
//! (`0600`): its organization's id and the domain users reach it at.
//! Owners and admins change the domain in Settings › Server; the command
//! line (`superfik-server config`) too, even while the server runs, so
//! every read takes the file again and a change holds at once.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use superfik_core::OrgId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerFile {
    pub org: OrgId,
    #[serde(default)]
    pub domain: Option<String>,
}

impl ServerFile {
    pub fn new(org: OrgId) -> Self {
        Self { org, domain: None }
    }

    pub fn shown(&self) -> superfik_protocol::ServerSettings {
        superfik_protocol::ServerSettings { domain: self.domain.clone() }
    }
}

#[derive(Debug)]
pub struct SettingsFile {
    path: Option<PathBuf>,
    last: ServerFile,
}

impl SettingsFile {
    pub fn in_memory(file: ServerFile) -> Self {
        Self { path: None, last: file }
    }

    pub fn open(path: &Path) -> std::io::Result<Self> {
        let last = match std::fs::read_to_string(path) {
            Ok(json) => serde_json::from_str(&json).map_err(std::io::Error::other)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let fresh = ServerFile::new(OrgId::new());
                write(path, &fresh)?;
                fresh
            }
            Err(error) => return Err(error),
        };
        Ok(Self { path: Some(path.to_path_buf()), last })
    }

    pub fn read(&mut self) -> ServerFile {
        if let Some(path) = &self.path
            && let Ok(json) = std::fs::read_to_string(path)
            && let Ok(file) = serde_json::from_str::<ServerFile>(&json)
        {
            self.last = file;
        }
        self.last.clone()
    }

    pub fn change(&mut self, change: impl FnOnce(&mut ServerFile)) -> std::io::Result<ServerFile> {
        let mut file = self.read();
        change(&mut file);
        if let Some(path) = &self.path {
            write(path, &file)?;
        }
        self.last = file.clone();
        Ok(file)
    }
}

fn write(path: &Path, file: &ServerFile) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(file).map_err(std::io::Error::other)?;
    superfik_private_file::replace(path, json.as_bytes())
}

pub fn given(text: &str) -> Option<String> {
    Some(text.trim().to_owned()).filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_made_once_and_changes_from_elsewhere_are_read() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("server.json");
        let mut server = SettingsFile::open(&path).expect("open");
        let org = server.read().org;
        assert_eq!(SettingsFile::open(&path).expect("again").read().org, org, "the id stays");
        SettingsFile::open(&path)
            .expect("command line")
            .change(|file| file.domain = Some("superfik.acme.io".into()))
            .expect("change");
        assert_eq!(server.read().domain.as_deref(), Some("superfik.acme.io"));
    }

    #[test]
    fn blank_text_clears_a_setting() {
        assert_eq!(given("  "), None);
        assert_eq!(given(" superfik.acme.io "), Some("superfik.acme.io".into()));
    }
}
