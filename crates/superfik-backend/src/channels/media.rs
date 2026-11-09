//! Files a channel account received, kept beside its session:
//! `<root>/<connection>/media/<kept name>` (`media::kept_name`, which no
//! sender's name can lead out of the folder). A file comes just before
//! its message, which then names where it was kept. What an agent may send
//! is a file in its run's project or one the account received.

use std::path::{Path, PathBuf};

use superfik_core::ConnectionId;
use superfik_core::channel::media::kept_name;
use superfik_core::channel::{Attachment, MAX_MEDIA_BYTES, Media, Message};

use super::{Channels, Observed};

impl Channels {
    pub fn media_dir(&self, connection: ConnectionId) -> PathBuf {
        self.inner.root.join(connection.to_string()).join("media")
    }

    pub(super) fn keep_file(&self, connection: ConnectionId, message: &str, media: &Media, bytes: &[u8]) -> Observed {
        if bytes.len() as u64 > MAX_MEDIA_BYTES {
            return Observed::default();
        }
        let dir = self.media_dir(connection);
        let kept =
            std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(kept_name(message, media)), bytes));
        if let Err(error) = kept {
            tracing::warn!(%error, "could not keep a received file");
        }
        Observed::default()
    }

    pub(super) fn with_kept_file(&self, connection: ConnectionId, mut message: Message) -> Message {
        if let Some(media) = message.media.as_mut() {
            let path = self.media_dir(connection).join(kept_name(&message.id, media));
            if let Ok(metadata) = std::fs::metadata(&path) {
                media.size = Some(metadata.len());
                media.file = Some(path);
            }
        }
        message
    }

    pub fn attachment(&self, connection: ConnectionId, project: &Path, path: &str) -> Result<Attachment, String> {
        let asked = Path::new(path.trim());
        let asked = if asked.is_absolute() { asked.to_path_buf() } else { project.join(asked) };
        let real = std::fs::canonicalize(&asked).map_err(|_| format!("There is no file at {}.", asked.display()))?;
        let within = |root: &Path| std::fs::canonicalize(root).is_ok_and(|root| real.starts_with(root));
        if !within(project) && !within(&self.media_dir(connection)) {
            return Err("Only files in the project, or ones this account received, can be sent.".to_owned());
        }
        let metadata = std::fs::metadata(&real).map_err(|error| error.to_string())?;
        if !metadata.is_file() {
            return Err(format!("{} is not a file.", real.display()));
        }
        if metadata.len() > MAX_MEDIA_BYTES {
            return Err(format!(
                "{} is larger than the {} MB a message can carry.",
                real.display(),
                MAX_MEDIA_BYTES >> 20
            ));
        }
        Ok(Attachment::of(real, metadata.len()))
    }
}

#[cfg(test)]
#[path = "tests_media.rs"]
mod tests;
