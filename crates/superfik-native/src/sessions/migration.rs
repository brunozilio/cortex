//! Offline, administrator-attested legacy imports. No network command accepts
//! this manifest: the operator supplies a private mapping outside runner access.
use super::{FileSessions, import_jsonl, scoped_id};
use crate::transcript::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use superfik_core::{EngineKind, ModelOption, Principal, PromptRequest, SessionId};

const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_BATCH_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub entries: Vec<Entry>,
}
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    #[default]
    ClaudeJsonl,
    NativeJson,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub owner: Principal,
    pub cwd: PathBuf,
    pub session_id: SessionId,
    pub source: PathBuf,
    pub sha256: String,
    #[serde(default)]
    pub format: SourceFormat,
}
#[derive(Debug)]
struct PreparedEntry {
    target: SessionId,
    history: Vec<Message>,
}
#[derive(Debug)]
pub struct Prepared {
    entries: Vec<PreparedEntry>,
}
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub imported: usize,
    pub already_present: usize,
}

pub fn prepare(manifest: &Path, source_root: &Path) -> Result<Prepared, String> {
    private(manifest, false)?;
    private(source_root, true)?;
    let root = source_root.canonicalize().map_err(|e| e.to_string())?;
    let mut manifest_bytes = Vec::new();
    fs::File::open(manifest)
        .map_err(|e| e.to_string())?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut manifest_bytes)
        .map_err(|e| e.to_string())?;
    if manifest_bytes.len() > 8 * 1024 * 1024 {
        return Err("migration manifest exceeds the 8 MiB limit".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("invalid migration manifest: {e}"))?;
    if manifest.version != 1 || manifest.entries.is_empty() || manifest.entries.len() > 10_000 {
        return Err("manifest requires version 1 and 1..10000 entries".into());
    }
    let mut targets = HashSet::new();
    let mut prepared = Vec::new();
    let mut total_bytes = 0;
    for (index, entry) in manifest.entries.into_iter().enumerate() {
        let mut validate = || -> Result<PreparedEntry, String> {
            if entry.session_id.0.is_empty()
                || !entry.session_id.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Err("invalid session id".into());
            }
            if !entry.cwd.is_absolute() || !entry.cwd.is_dir() {
                return Err("cwd must be an existing absolute project directory".into());
            }
            if entry.source.as_os_str().is_empty()
                || !entry.source.components().all(|p| matches!(p, Component::Normal(_)))
            {
                return Err("source must be a relative export path without traversal".into());
            }
            let source = root.join(&entry.source).canonicalize().map_err(|e| e.to_string())?;
            if !source.starts_with(&root) {
                return Err("source symlink escapes the export directory".into());
            }
            let metadata = fs::metadata(&source).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES {
                return Err("source is not a regular transcript within the 128 MiB limit".into());
            }
            let mut bytes = Vec::new();
            fs::File::open(&source)
                .map_err(|e| e.to_string())?
                .take(MAX_SOURCE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_SOURCE_BYTES {
                return Err("source exceeds the transcript limit".into());
            }
            total_bytes += bytes.len() as u64;
            if total_bytes > MAX_BATCH_BYTES {
                return Err("migration batch exceeds 256 MiB; split the reviewed manifest".into());
            }
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if actual != entry.sha256.to_ascii_lowercase() {
                return Err("source SHA-256 does not match the reviewed manifest".into());
            }
            let text = std::str::from_utf8(&bytes).map_err(|_| "source is not UTF-8")?;
            let history = match entry.format {
                SourceFormat::ClaudeJsonl => import_jsonl(text)?,
                SourceFormat::NativeJson => {
                    let messages: Vec<Message> =
                        serde_json::from_str(text).map_err(|e| format!("invalid native transcript: {e}"))?;
                    if messages.is_empty() {
                        return Err("no conversation in the transcript".into());
                    }
                    messages
                }
            };
            let mut request = PromptRequest::plain(
                entry.cwd.clone(),
                "",
                ModelOption::new(EngineKind::Native, "migration", "Migration"),
            );
            request.credentials.owner = Some(entry.owner.clone());
            Ok(PreparedEntry { target: scoped_id(&request, &entry.session_id), history })
        };
        let entry = validate().map_err(|error| format!("entry {}: {error}", index + 1))?;
        if !targets.insert(entry.target.clone()) {
            return Err(format!("entry {} repeats a target scope", index + 1));
        }
        prepared.push(entry);
    }
    Ok(Prepared { entries: prepared })
}
impl Prepared {
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn apply(&self, sessions: &FileSessions) -> Result<Report, String> {
        fs::create_dir_all(&sessions.dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&sessions.dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        }
        for entry in &self.entries {
            let path = sessions.path(&entry.target).ok_or("invalid target")?;
            match fs::read(path) {
                Ok(bytes) if !same_history(&bytes, &entry.history) => {
                    return Err("target contains a different conversation; refusing to overwrite it".into());
                }
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
                _ => {}
            }
        }
        let mut report = Report::default();
        for entry in &self.entries {
            let path = sessions.path(&entry.target).ok_or("invalid target")?;
            let bytes = serde_json::to_vec(&entry.history).map_err(|e| e.to_string())?;
            let temporary = sessions.dir.join(format!("import-{}.partial", uuid::Uuid::new_v4()));
            let result = (|| -> Result<bool, String> {
                let mut file =
                    superfik_private_file::open(fs::OpenOptions::new().write(true).create_new(true), &temporary)
                        .map_err(|e| e.to_string())?;
                file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
                match fs::hard_link(&temporary, &path) {
                    Ok(()) => Ok(true),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if same_history(&fs::read(&path).map_err(|e| e.to_string())?, &entry.history) {
                            Ok(false)
                        } else {
                            Err("target changed during import; refusing to overwrite it".into())
                        }
                    }
                    Err(e) => Err(e.to_string()),
                }
            })();
            let _ = fs::remove_file(&temporary);
            match result? {
                true => report.imported += 1,
                false => report.already_present += 1,
            }
        }
        Ok(report)
    }
}
fn private(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    if directory != metadata.is_dir() || (!directory && !metadata.is_file()) {
        return Err("migration input has the wrong file type".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("manifest and export directory must be private (0600/0700)".into());
        }
    }
    Ok(())
}

fn same_history(bytes: &[u8], expected: &[Message]) -> bool {
    serde_json::from_slice::<Vec<Message>>(bytes).is_ok_and(|history| history == expected)
}
