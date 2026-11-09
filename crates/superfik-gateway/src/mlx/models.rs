//! What is downloaded: one folder per Hugging Face id (`<org>/<name>`)
//! under the models folder, offered once its download is complete.

use std::path::Path;

use superfik_core::{EngineKind, ModelOption, ModelProvider};

pub fn downloaded(root: &Path, engine: EngineKind) -> Vec<ModelOption> {
    let mut ids: Vec<String> = subfolders(root)
        .into_iter()
        .flat_map(|org| subfolders(&org).into_iter().map(move |model| (org.clone(), model)))
        .filter(|(_, model)| complete(model))
        .filter_map(|(org, model)| Some(format!("{}/{}", org.file_name()?.to_str()?, model.file_name()?.to_str()?)))
        .collect();
    ids.sort();
    ids.into_iter()
        .map(|id| {
            let label = label(&id);
            let window = window(&root.join(&id));
            let model = ModelOption::new(engine, id, label).served_by(ModelProvider::Local);
            match window {
                Some(tokens) => model.with_window(tokens),
                None => model,
            }
        })
        .collect()
}

fn window(model: &Path) -> Option<u64> {
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(model.join("config.json")).ok()?).ok()?;
    let key = "max_position_embeddings";
    config[key].as_u64().or_else(|| config["text_config"][key].as_u64()).filter(|tokens| *tokens > 0)
}

fn subfolders(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && !path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect()
}

pub(super) fn complete(model: &Path) -> bool {
    let partial = std::fs::read_dir(model.join(".cache/huggingface/download"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "incomplete"));
    model.join("config.json").is_file() && !partial
}

pub(super) fn bytes_in(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => bytes_in(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |meta| meta.len()),
            _ => 0,
        })
        .sum()
}

pub fn repository_size(listing: &str) -> Option<u64> {
    let entries: Vec<serde_json::Value> = serde_json::from_str(listing).ok()?;
    let sizes = entries.iter().filter(|entry| entry["type"] == "file").map(|entry| entry["size"].as_u64());
    sizes.sum::<Option<u64>>().filter(|total| *total > 0)
}

fn label(id: &str) -> String {
    let name = id.rsplit('/').next().unwrap_or(id);
    name.split('-')
        .map(|part| match part.strip_suffix("bit") {
            Some(bits) if bits.chars().all(|c| c.is_ascii_digit()) && !bits.is_empty() => format!("{bits}-bit"),
            _ => part.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}
