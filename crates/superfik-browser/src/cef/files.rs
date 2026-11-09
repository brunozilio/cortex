//! Conversation-owned file staging and native download observations.
use serde::Serialize;
use serde_json::json;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use superfik_core::BrowserAnswer;
use welding::NavigationEvent;

#[derive(Debug, Clone)]
pub(super) struct FileArea {
    root: PathBuf,
    transfers: Rc<RefCell<VecDeque<Transfer>>>,
}
#[derive(Debug, Serialize)]
struct Transfer {
    id: u32,
    name: String,
    path: PathBuf,
    state: &'static str,
    bytes: u64,
    error: Option<String>,
}

impl FileArea {
    pub fn prepare(root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(root.join("uploads")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(root.join("downloads")).map_err(|e| e.to_string())?;
        Ok(Self { root: root.canonicalize().map_err(|e| e.to_string())?, transfers: Rc::default() })
    }
    pub fn downloads(&self) -> PathBuf {
        self.root.join("downloads")
    }
    pub fn uploads(&self, paths: &[String]) -> Result<Vec<String>, String> {
        if paths.len() > 16 {
            return Err("Attach at most 16 files per operation.".into());
        }
        let root = self.root.join("uploads").canonicalize().map_err(|e| e.to_string())?;
        paths
            .iter()
            .map(|value| {
                let requested = Path::new(value);
                if value.is_empty() || value.len() > 4096 || requested.components().any(|c| c == Component::ParentDir) {
                    return Err("Upload paths must stay inside the upload directory.".into());
                }
                let path = if requested.is_absolute() { requested.to_path_buf() } else { root.join(requested) };
                let path = path.canonicalize().map_err(|e| format!("Cannot open upload: {e}"))?;
                if !path.starts_with(&root) || !path.is_file() {
                    return Err("Upload resolves outside the staging directory or is not a file.".into());
                }
                path.to_str().map(str::to_owned).ok_or("Upload path is not UTF-8.".into())
            })
            .collect()
    }
    pub fn summary(&self) -> Result<BrowserAnswer, String> {
        let saved: Vec<_> = std::fs::read_dir(self.downloads())
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .take(128)
            .map(|entry| entry.path())
            .collect();
        Ok(BrowserAnswer::text(json!({"upload_directory":self.root.join("uploads"),"download_directory":self.downloads(),"downloads":*self.transfers.borrow(),"saved_files":saved}).to_string()))
    }
    pub fn observe(&self, event: &NavigationEvent) {
        let mut transfers = self.transfers.borrow_mut();
        match event {
            NavigationEvent::DownloadStarted { id, suggested_filename, destination_path, .. } => {
                if transfers.len() >= 128 {
                    transfers.pop_front();
                }
                transfers.push_back(Transfer {
                    id: *id,
                    name: suggested_filename.clone(),
                    path: destination_path.clone(),
                    state: "running",
                    bytes: 0,
                    error: None,
                });
            }
            NavigationEvent::DownloadProgress { id, bytes_received, .. } => {
                if let Some(item) = transfers.iter_mut().find(|item| item.id == *id) {
                    item.bytes = *bytes_received;
                }
            }
            NavigationEvent::DownloadFinished { id, destination_path, error } => {
                if let Some(item) = transfers.iter_mut().find(|item| item.id == *id) {
                    item.path = destination_path.clone();
                    item.state = if error.is_some() { "failed" } else { "complete" };
                    item.error = error.clone();
                }
            }
            NavigationEvent::DownloadCancelled { id, .. } => {
                if let Some(item) = transfers.iter_mut().find(|item| item.id == *id) {
                    item.state = "cancelled";
                }
            }
            _ => {}
        }
    }
}
