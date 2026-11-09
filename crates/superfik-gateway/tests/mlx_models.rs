//! The local models offered are the complete downloads in the models folder.

use std::path::Path;

use superfik_core::{EngineKind, ModelProvider};
use superfik_gateway::mlx::{downloaded, repository_size};

fn model(root: &Path, id: &str) {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("config.json"), "{}").expect("config");
}

#[test]
fn complete_downloads_are_offered_as_local_models_of_the_engine_asked() {
    let root = tempfile::tempdir().expect("root");
    model(root.path(), "mlx-community/Qwen3.8-27B-8bit");
    model(root.path(), "mlx-community/Qwen3-Coder-Next-4bit");
    let models = downloaded(root.path(), EngineKind::Native);
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["mlx-community/Qwen3-Coder-Next-4bit", "mlx-community/Qwen3.8-27B-8bit"]);
    assert!(models.iter().all(|m| m.engine == EngineKind::Native && m.provider == ModelProvider::Local));
    assert_eq!(models[1].label, "Qwen3.8 27B 8-bit");
}

#[test]
fn a_download_still_running_is_not_offered() {
    let root = tempfile::tempdir().expect("root");
    model(root.path(), "mlx-community/Qwen3.8-27B-8bit");
    let partial = root.path().join("mlx-community/Qwen3.8-27B-8bit/.cache/huggingface/download");
    std::fs::create_dir_all(&partial).expect("cache");
    std::fs::write(partial.join("model-00003-of-00006.safetensors.incomplete"), "").expect("partial");
    assert!(downloaded(root.path(), EngineKind::Native).is_empty());
}

#[test]
fn no_models_folder_offers_nothing() {
    assert!(downloaded(Path::new("/nonexistent/superfik/models"), EngineKind::Native).is_empty());
}

#[test]
fn a_model_s_context_window_comes_from_its_config() {
    let root = tempfile::tempdir().expect("root");
    model(root.path(), "mlx-community/Plain");
    model(root.path(), "mlx-community/Qwen3.8-27B-8bit");
    let config = r#"{"model_type": "qwen3_5", "text_config": {"max_position_embeddings": 262144}}"#;
    std::fs::write(root.path().join("mlx-community/Qwen3.8-27B-8bit/config.json"), config).expect("config");
    let models = downloaded(root.path(), EngineKind::Native);
    assert_eq!(models[0].window, None, "a config that says nothing");
    assert_eq!(models[1].window, Some(262_144));
}

#[test]
fn a_repository_weighs_what_its_files_do() {
    let listing = r#"[{"type":"file","size":939,"path":"config.json"},
        {"type":"directory","size":0,"path":"extra"},
        {"type":"file","size":4607835174,"lfs":{"size":4607835174},"path":"model.safetensors"}]"#;
    assert_eq!(repository_size(listing), Some(4_607_836_113));
    assert_eq!(repository_size("[]"), None, "an empty listing says nothing");
    assert_eq!(repository_size("{\"error\":\"Repository not found\"}"), None);
}
