//! Models pulled from Hugging Face like `ollama pull`: only the files
//! `mlx_lm` needs, put together in a hidden `.partial` folder and moved
//! into place when complete, so a cancelled pull is never listed. And
//! removed again, saying how much space came back.

mod files;
mod hf;
mod repo;
#[cfg(test)]
mod tests;

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

pub use files::RemoteFile;
pub use hf::HfHub;
pub use repo::{DEFAULT_ORG, RepoId};

#[derive(Debug, thiserror::Error)]
pub enum LocalModelError {
    #[error("local MLX models need Apple silicon")]
    NeedsAppleSilicon,
    #[error("this computer has no folder for Superfik's local data")]
    NoDataFolder,
    #[error("`{id}` is not a Hugging Face model id ({why}); use <org>/<name>, like mlx-community/Qwen3-8B-4bit")]
    InvalidId { id: String, why: String },
    #[error("{0} is not on Hugging Face")]
    NotFound(String),
    #[error("{0} is private, gated or not on Hugging Face; for a gated model, accept its terms there and set HF_TOKEN")]
    Unavailable(String),
    #[error("{id} is not an MLX model: {why}")]
    NotMlx { id: String, why: String },
    #[error("{0} is not downloaded")]
    NotDownloaded(String),
    #[error("could not download {id}: {why}")]
    Network { id: String, why: String },
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> LocalModelError + '_ {
    move |source| LocalModelError::Io { path: path.to_owned(), source }
}

/// Where models come from (Hugging Face; a fake in tests).
pub trait Hub {
    /// Every file in the model's repository.
    fn files(&self, id: &RepoId) -> Result<Vec<RemoteFile>, LocalModelError>;
    /// Writes `file` to `sink`, telling `progress` each run of bytes written.
    fn download(
        &self,
        id: &RepoId,
        file: &str,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), LocalModelError>;
}

/// How far the pull of model `id` is: the file being downloaded (`index`
/// of `files`) and the bytes done of all of them (`total` when the hub
/// gave every size).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullProgress {
    pub id: String,
    pub file: String,
    pub index: usize,
    pub files: usize,
    pub done: u64,
    pub total: Option<u64>,
}

/// A model in place: `fresh` when this pull downloaded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pulled {
    pub id: String,
    pub folder: PathBuf,
    pub bytes: u64,
    pub fresh: bool,
}

/// A model removed and the space it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub id: String,
    pub folder: PathBuf,
    pub bytes: u64,
}

/// Downloads `model` into `root` (the MLX models folder) unless it is
/// already there.
pub fn pull(
    hub: &dyn Hub,
    root: &Path,
    model: &str,
    progress: &mut dyn FnMut(&PullProgress),
) -> Result<Pulled, LocalModelError> {
    let id = RepoId::parse(model)?;
    let folder = id.folder(root);
    if crate::models::complete(&folder) {
        return Ok(Pulled { id: id.to_string(), bytes: size(&folder), folder, fresh: false });
    }
    let files = files::wanted(&hub.files(&id)?);
    files::check(&id, &files)?;
    let partial = id.partial(root);
    let fetched = fetch_all(hub, &id, &files, &partial, progress).and_then(|bytes| {
        if folder.exists() {
            std::fs::remove_dir_all(&folder).map_err(io_at(&folder))?;
        }
        std::fs::rename(&partial, &folder).map_err(io_at(&folder))?;
        Ok(bytes)
    });
    match fetched {
        Ok(bytes) => Ok(Pulled { id: id.to_string(), folder, bytes, fresh: true }),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&partial);
            Err(error)
        }
    }
}

fn fetch_all(
    hub: &dyn Hub,
    id: &RepoId,
    files: &[RemoteFile],
    partial: &Path,
    progress: &mut dyn FnMut(&PullProgress),
) -> Result<u64, LocalModelError> {
    if partial.exists() {
        std::fs::remove_dir_all(partial).map_err(io_at(partial))?;
    }
    std::fs::create_dir_all(partial).map_err(io_at(partial))?;
    let total = files.iter().map(|file| file.size).sum::<Option<u64>>();
    let mut state =
        PullProgress { id: id.to_string(), file: String::new(), index: 0, files: files.len(), done: 0, total };
    for (index, file) in files.iter().enumerate() {
        state.file.clone_from(&file.name);
        state.index = index;
        progress(&state);
        let path = partial.join(&file.name);
        let mut sink = BufWriter::new(std::fs::File::create(&path).map_err(io_at(&path))?);
        let mut written = 0;
        hub.download(id, &file.name, &mut sink, &mut |bytes| {
            written += bytes;
            state.done += bytes;
            progress(&state);
        })?;
        sink.into_inner().map_err(|e| io_at(&path)(e.into_error()))?.sync_all().map_err(io_at(&path))?;
        if file.size.is_some_and(|size| size != written) {
            let why = format!("{} was cut short ({written} of {} bytes)", file.name, file.size.unwrap_or_default());
            return Err(LocalModelError::Network { id: id.to_string(), why });
        }
    }
    Ok(state.done)
}

/// Deletes `model` from `root`, with any download of it left unfinished.
pub fn remove(root: &Path, model: &str) -> Result<Removed, LocalModelError> {
    let id = RepoId::parse(model)?;
    let folder = id.folder(root);
    let partial = id.partial(root);
    if !folder.exists() && !partial.exists() {
        return Err(LocalModelError::NotDownloaded(id.to_string()));
    }
    let mut bytes = 0;
    for dir in [&folder, &partial] {
        if dir.exists() {
            bytes += size(dir);
            std::fs::remove_dir_all(dir).map_err(io_at(dir))?;
        }
    }
    if let Some(org) = folder.parent() {
        let _ = std::fs::remove_dir(org);
    }
    Ok(Removed { id: id.to_string(), folder, bytes })
}

/// The bytes under `path`, links counted as themselves.
fn size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(path).into_iter().flatten().flatten().map(|entry| size(&entry.path())).sum()
}
