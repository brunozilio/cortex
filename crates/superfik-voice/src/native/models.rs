//! The models on disk: where each lives under the app's models folder, and
//! fetching one the first time it is needed (sherpa-onnx's `.tar.bz2`
//! releases, unpacked beside and moved in whole, so a cut download never
//! looks complete).

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::{KOKORO_FOLDER, Recognizer, VAD_FILE, VoiceProblem, kokoro_url, vad_url};

const RECOGNIZER_FILES: [&str; 4] = ["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"];
const KOKORO_FILES: [&str; 4] = ["model.onnx", "voices.bin", "tokens.txt", "lexicon-us-en.txt"];

#[derive(Debug, Clone)]
pub struct Models {
    root: PathBuf,
}

impl Models {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn recognizer(&self, recognizer: Recognizer) -> PathBuf {
        self.root.join(recognizer.folder())
    }

    pub fn kokoro(&self) -> PathBuf {
        self.root.join(KOKORO_FOLDER)
    }

    pub fn has_recognizer(&self, recognizer: Recognizer) -> bool {
        complete(&self.recognizer(recognizer), &RECOGNIZER_FILES)
    }

    pub fn has_kokoro(&self) -> bool {
        complete(&self.kokoro(), &KOKORO_FILES)
    }

    pub fn fetch_recognizer(&self, recognizer: Recognizer, progress: impl FnMut(f32)) -> Result<(), VoiceProblem> {
        if self.has_recognizer(recognizer) {
            return Ok(());
        }
        self.fetch(&recognizer.url(), &recognizer.folder(), progress)?;
        check(self.has_recognizer(recognizer), &recognizer.folder())
    }

    pub fn fetch_kokoro(&self, progress: impl FnMut(f32)) -> Result<(), VoiceProblem> {
        if self.has_kokoro() {
            return Ok(());
        }
        self.fetch(&kokoro_url(), KOKORO_FOLDER, progress)?;
        check(self.has_kokoro(), KOKORO_FOLDER)
    }

    pub fn vad(&self) -> PathBuf {
        self.root.join(VAD_FILE)
    }

    pub fn has_vad(&self) -> bool {
        self.vad().is_file()
    }

    pub fn fetch_vad(&self) -> Result<(), VoiceProblem> {
        if self.has_vad() {
            return Ok(());
        }
        let fail = |why: String| VoiceProblem::Download(format!("{VAD_FILE}: {why}"));
        std::fs::create_dir_all(&self.root).map_err(|e| fail(e.to_string()))?;
        let partial = self.root.join(format!(".partial-{VAD_FILE}"));
        tracing::info!(url = %vad_url(), "downloading the voice activity detector");
        let mut response = ureq::get(&vad_url()).call().map_err(|e| fail(e.to_string()))?;
        let mut file = std::fs::File::create(&partial).map_err(|e| fail(e.to_string()))?;
        std::io::copy(&mut response.body_mut().as_reader(), &mut file).map_err(|e| fail(e.to_string()))?;
        std::fs::rename(&partial, self.vad()).map_err(|e| fail(e.to_string()))
    }

    fn fetch(&self, url: &str, folder: &str, mut progress: impl FnMut(f32)) -> Result<(), VoiceProblem> {
        let fail = |why: String| VoiceProblem::Download(format!("{folder}: {why}"));
        std::fs::create_dir_all(&self.root).map_err(|e| fail(e.to_string()))?;
        let partial = self.root.join(format!(".partial-{folder}"));
        let _ = std::fs::remove_dir_all(&partial);
        tracing::info!(%url, "downloading a voice model");
        let mut response = ureq::get(url).call().map_err(|e| fail(e.to_string()))?;
        let total =
            response.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
        let body = Counted { inner: response.body_mut().as_reader(), read: 0, total, progress: &mut progress };
        let archive = bzip2::read::BzDecoder::new(body);
        tar::Archive::new(archive).unpack(&partial).map_err(|e| fail(e.to_string()))?;
        let unpacked = partial.join(folder);
        let target = self.root.join(folder);
        let _ = std::fs::remove_dir_all(&target);
        std::fs::rename(&unpacked, &target).map_err(|e| fail(e.to_string()))?;
        let _ = std::fs::remove_dir_all(&partial);
        progress(1.0);
        Ok(())
    }
}

fn complete(folder: &Path, files: &[&str]) -> bool {
    files.iter().all(|f| folder.join(f).is_file())
}

fn check(complete: bool, folder: &str) -> Result<(), VoiceProblem> {
    if complete { Ok(()) } else { Err(VoiceProblem::Download(format!("{folder}: files missing after unpacking"))) }
}

struct Counted<'a, R, F> {
    inner: R,
    read: u64,
    total: Option<u64>,
    progress: &'a mut F,
}

impl<R: Read, F: FnMut(f32)> Read for Counted<'_, R, F> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        let before = self.read;
        self.read += n as u64;
        if let Some(total) = self.total.filter(|&t| t > 0)
            && self.read * 100 / total != before * 100 / total
        {
            (self.progress)((self.read as f32 / total as f32).min(0.99));
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_counts_only_once_every_file_is_there() {
        let dir = tempfile::tempdir().expect("test");
        let models = Models::new(dir.path().to_owned());
        let recognizer = Recognizer::default();
        assert!(!models.has_recognizer(recognizer));
        let folder = models.recognizer(recognizer);
        std::fs::create_dir_all(&folder).expect("test");
        for file in &RECOGNIZER_FILES[..3] {
            std::fs::write(folder.join(file), b"").expect("test");
        }
        assert!(!models.has_recognizer(recognizer), "tokens.txt is missing");
        std::fs::write(folder.join("tokens.txt"), b"").expect("test");
        assert!(models.has_recognizer(recognizer));
        assert!(models.fetch_recognizer(recognizer, |_| {}).is_ok(), "nothing to download");
    }

    #[test]
    fn the_detector_is_one_file_kept_beside_the_models() {
        let dir = tempfile::tempdir().expect("test");
        let models = Models::new(dir.path().to_owned());
        assert!(!models.has_vad());
        assert_eq!(models.vad(), dir.path().join(crate::VAD_FILE));
        std::fs::write(models.vad(), b"onnx").expect("test");
        assert!(models.has_vad());
        assert!(models.fetch_vad().is_ok(), "nothing to download");
    }

    #[test]
    fn progress_is_reported_as_the_body_is_read() {
        let mut seen = Vec::new();
        let mut progress = |f: f32| seen.push(f);
        let data = vec![0u8; 1000];
        let mut reader = Counted { inner: &data[..], read: 0, total: Some(1000), progress: &mut progress };
        let mut buf = [0u8; 250];
        while reader.read(&mut buf).expect("test") > 0 {}
        assert_eq!(seen, [0.25, 0.5, 0.75, 0.99]);
    }
}
