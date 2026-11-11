use std::collections::BTreeMap;
use std::sync::Mutex;

use super::*;

/// A hub serving files from memory; `broken` fails mid-download.
#[derive(Default)]
struct FakeHub {
    files: BTreeMap<String, Vec<u8>>,
    broken: Option<String>,
    asked: Mutex<Vec<String>>,
}

impl FakeHub {
    fn with(files: &[(&str, &str)]) -> Self {
        let files = files.iter().map(|(name, body)| ((*name).to_owned(), body.as_bytes().to_vec())).collect();
        Self { files, ..Self::default() }
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("asked").clone()
    }
}

impl Hub for FakeHub {
    fn files(&self, _: &RepoId) -> Result<Vec<RemoteFile>, LocalModelError> {
        Ok(self.files.iter().map(|(name, body)| RemoteFile::new(name, Some(body.len() as u64))).collect())
    }

    fn download(
        &self,
        id: &RepoId,
        file: &str,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), LocalModelError> {
        self.asked.lock().expect("asked").push(file.to_owned());
        let body = self.files.get(file).ok_or_else(|| LocalModelError::NotFound(file.to_owned()))?;
        for chunk in body.chunks(2) {
            sink.write_all(chunk).map_err(|e| LocalModelError::Io { path: file.into(), source: e })?;
            progress(chunk.len() as u64);
        }
        if self.broken.as_deref() == Some(file) {
            return Err(LocalModelError::Network { id: id.to_string(), why: "connection reset".into() });
        }
        Ok(())
    }
}

fn model() -> FakeHub {
    FakeHub::with(&[
        ("README.md", "# hi"),
        ("config.json", "{\"max_position_embeddings\":4096}"),
        ("model.safetensors", "weights"),
        ("tokenizer.json", "{}"),
    ])
}

fn listed(root: &Path) -> Vec<String> {
    crate::downloaded(root).into_iter().map(|model| model.id).collect()
}

#[test]
fn a_pulled_model_is_listed_with_only_the_files_mlx_needs() {
    let root = tempfile::tempdir().expect("root");
    let hub = model();
    let mut seen = Vec::new();
    let pulled = pull(&hub, root.path(), "Tiny-4bit", &mut |p| seen.push(p.clone())).expect("pull");
    assert_eq!(pulled.id, "mlx-community/Tiny-4bit");
    assert!(pulled.fresh);
    assert_eq!(pulled.bytes, 32 + 7 + 2);
    assert_eq!(hub.asked(), ["config.json", "model.safetensors", "tokenizer.json"]);
    assert_eq!(listed(root.path()), ["mlx-community/Tiny-4bit"]);
    let folder = root.path().join("mlx-community/Tiny-4bit");
    assert_eq!(pulled.folder, folder);
    assert_eq!(std::fs::read_to_string(folder.join("model.safetensors")).expect("weights"), "weights");
    assert!(!folder.join("README.md").exists());
    assert!(!root.path().join("mlx-community/.Tiny-4bit.partial").exists(), "the partial folder is moved");
    let last = seen.last().expect("progress");
    assert_eq!((last.done, last.total, last.index, last.files), (41, Some(41), 2, 3));
    assert!(seen.windows(2).all(|pair| pair[0].done <= pair[1].done), "progress only goes forward");
}

#[test]
fn a_failed_pull_leaves_nothing_behind_to_list() {
    let root = tempfile::tempdir().expect("root");
    let hub = FakeHub { broken: Some("tokenizer.json".into()), ..model() };
    let failed = pull(&hub, root.path(), "mlx-community/Tiny-4bit", &mut |_| {}).expect_err("broken");
    assert!(failed.to_string().contains("connection reset"), "{failed}");
    assert!(listed(root.path()).is_empty());
    assert!(!root.path().join("mlx-community/.Tiny-4bit.partial").exists());
    assert!(!root.path().join("mlx-community/Tiny-4bit").exists());
}

#[test]
fn a_partial_folder_from_a_cancelled_pull_is_never_listed_and_is_replaced() {
    let root = tempfile::tempdir().expect("root");
    let leftover = root.path().join("mlx-community/.Tiny-4bit.partial");
    std::fs::create_dir_all(&leftover).expect("leftover");
    std::fs::write(leftover.join("config.json"), "{}").expect("config");
    std::fs::write(leftover.join("stale.safetensors"), "old").expect("stale");
    assert!(listed(root.path()).is_empty(), "a cancelled pull is not offered");
    pull(&model(), root.path(), "Tiny-4bit", &mut |_| {}).expect("pull");
    let folder = root.path().join("mlx-community/Tiny-4bit");
    assert!(!folder.join("stale.safetensors").exists(), "the leftover is started over");
    assert!(!leftover.exists());
}

#[test]
fn a_model_already_downloaded_is_not_fetched_again() {
    let root = tempfile::tempdir().expect("root");
    let folder = root.path().join("mlx-community/Tiny-4bit");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("config.json"), "{}").expect("config");
    let hub = model();
    let pulled = pull(&hub, root.path(), "Tiny-4bit", &mut |_| {}).expect("pull");
    assert!(!pulled.fresh);
    assert_eq!(pulled.bytes, 2);
    assert!(hub.asked().is_empty());
}

#[test]
fn a_repository_that_is_not_an_mlx_model_is_refused_before_downloading() {
    let root = tempfile::tempdir().expect("root");
    let hub = FakeHub::with(&[("README.md", "x"), ("model.gguf", "x")]);
    let refused = pull(&hub, root.path(), "org/gguf-only", &mut |_| {}).expect_err("not mlx");
    assert!(matches!(refused, LocalModelError::NotMlx { .. }), "{refused:?}");
    assert!(hub.asked().is_empty());
    assert_eq!(std::fs::read_dir(root.path()).expect("root").count(), 0, "nothing is written");
}

#[test]
fn a_file_shorter_than_the_listing_said_fails_the_pull() {
    struct Short(FakeHub);
    impl Hub for Short {
        fn files(&self, id: &RepoId) -> Result<Vec<RemoteFile>, LocalModelError> {
            let mut files = self.0.files(id)?;
            files.iter_mut().for_each(|file| file.size = file.size.map(|size| size + 1));
            Ok(files)
        }
        fn download(
            &self,
            id: &RepoId,
            file: &str,
            sink: &mut dyn Write,
            progress: &mut dyn FnMut(u64),
        ) -> Result<(), LocalModelError> {
            self.0.download(id, file, sink, progress)
        }
    }
    let root = tempfile::tempdir().expect("root");
    let short = pull(&Short(model()), root.path(), "Tiny-4bit", &mut |_| {}).expect_err("short");
    assert!(short.to_string().contains("cut short"), "{short}");
    assert!(listed(root.path()).is_empty());
}

#[test]
fn remove_deletes_the_model_and_says_how_much_it_freed() {
    let root = tempfile::tempdir().expect("root");
    pull(&model(), root.path(), "Tiny-4bit", &mut |_| {}).expect("pull");
    let removed = remove(root.path(), "mlx-community/Tiny-4bit").expect("remove");
    assert_eq!((removed.id.as_str(), removed.bytes), ("mlx-community/Tiny-4bit", 41));
    assert!(listed(root.path()).is_empty());
    assert!(!root.path().join("mlx-community").exists(), "an emptied organisation folder goes too");
    let again = remove(root.path(), "Tiny-4bit").expect_err("gone");
    assert!(matches!(again, LocalModelError::NotDownloaded(_)), "{again:?}");
    assert!(matches!(remove(root.path(), "../x"), Err(LocalModelError::InvalidId { .. })));
}

#[test]
fn remove_keeps_the_organisations_other_models() {
    let root = tempfile::tempdir().expect("root");
    pull(&model(), root.path(), "Tiny-4bit", &mut |_| {}).expect("pull");
    pull(&model(), root.path(), "Other-8bit", &mut |_| {}).expect("pull");
    remove(root.path(), "Tiny-4bit").expect("remove");
    assert_eq!(listed(root.path()), ["mlx-community/Other-8bit"]);
}
