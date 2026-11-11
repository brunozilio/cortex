//! Local MLX models on this computer, like `ollama pull` / `rm`: pulled
//! from Hugging Face into the models folder and removed again.
//!
//! These touch only this computer's folder (Apple silicon), so they are
//! not backend commands; the backend lists what is there on its own
//! (`ListModels`), so a pulled model shows up in [`crate::Sdk::models`].

use std::path::{Path, PathBuf};

use backend::local_models;
pub use backend::local_models::{LocalModelError, PullProgress, Pulled, Removed};

#[derive(Debug, Clone)]
pub struct LocalModels {
    folder: PathBuf,
}

impl LocalModels {
    /// This computer's MLX models folder; refused off Apple silicon.
    pub fn system() -> Result<Self, LocalModelError> {
        Ok(Self { folder: local_models::folder()? })
    }

    /// Models kept in `folder` instead (tests, or another profile).
    pub fn in_folder(folder: impl Into<PathBuf>) -> Self {
        Self { folder: folder.into() }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Downloads `model` (`<org>/<name>`, or a bare name for
    /// `mlx-community/<name>`) from Hugging Face, sending `HF_TOKEN` when
    /// set; `progress` hears each step.
    pub fn pull(&self, model: &str, mut progress: impl FnMut(&PullProgress)) -> Result<Pulled, LocalModelError> {
        let hub = local_models::HfHub::from_env()?;
        local_models::pull(&hub, &self.folder, model, &mut progress)
    }

    /// Deletes a downloaded model and says how much space it took.
    pub fn remove(&self, model: &str) -> Result<Removed, LocalModelError> {
        local_models::remove(&self.folder, model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removed_model_says_what_it_freed() {
        let root = tempfile::tempdir().expect("root");
        let folder = root.path().join("mlx-community/Tiny-4bit");
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("config.json"), "{}").expect("config");
        let models = LocalModels::in_folder(root.path());
        let removed = models.remove("Tiny-4bit").expect("removed");
        assert_eq!((removed.id.as_str(), removed.bytes), ("mlx-community/Tiny-4bit", 2));
        assert!(matches!(models.remove("Tiny-4bit"), Err(LocalModelError::NotDownloaded(_))));
    }

    #[test]
    fn an_invalid_id_is_refused_before_any_download() {
        let root = tempfile::tempdir().expect("root");
        let refused = LocalModels::in_folder(root.path()).pull("../../etc", |_| {}).expect_err("refused");
        assert!(matches!(refused, LocalModelError::InvalidId { .. }), "{refused:?}");
    }

    #[test]
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    fn local_models_need_apple_silicon() {
        assert!(matches!(LocalModels::system(), Err(LocalModelError::NeedsAppleSilicon)));
    }
}
