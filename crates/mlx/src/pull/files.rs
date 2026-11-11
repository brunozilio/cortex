//! Which of a repository's files `mlx_lm` needs: its config, tokenizer,
//! chat template and weights, never the README, images or Git metadata.

use serde_json::Value;

use super::{LocalModelError, RepoId};

/// A file in a model's repository, with its size when the hub says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub name: String,
    pub size: Option<u64>,
}

impl RemoteFile {
    pub fn new(name: impl Into<String>, size: Option<u64>) -> Self {
        Self { name: name.into(), size }
    }
}

const KEPT: [&str; 6] = ["json", "safetensors", "jinja", "model", "tiktoken", "txt"];

/// The files of a model listing (`GET /api/models/<repo>`): its `siblings`.
pub fn parse_listing(json: &str) -> Result<Vec<RemoteFile>, String> {
    let listing: Value = serde_json::from_str(json).map_err(|e| format!("the model listing is not JSON: {e}"))?;
    let siblings = listing["siblings"].as_array().ok_or("the model listing has no files")?;
    Ok(siblings
        .iter()
        .filter_map(|file| Some(RemoteFile::new(file["rfilename"].as_str()?, file["size"].as_u64())))
        .collect())
}

/// The files to download, in the listing's order: top-level, safely
/// named, of a kind `mlx_lm` reads.
pub fn wanted(files: &[RemoteFile]) -> Vec<RemoteFile> {
    files.iter().filter(|file| needed(&file.name)).cloned().collect()
}

fn needed(name: &str) -> bool {
    let safe = !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !name.contains("..");
    safe && name.rsplit_once('.').is_some_and(|(_, ext)| KEPT.contains(&ext))
}

/// Refuses a repository `mlx_lm` could not load.
pub fn check(id: &RepoId, files: &[RemoteFile]) -> Result<(), LocalModelError> {
    let not_mlx = |why: &str| LocalModelError::NotMlx { id: id.to_string(), why: why.to_owned() };
    if !files.iter().any(|file| file.name == "config.json") {
        return Err(not_mlx("it has no config.json"));
    }
    if !files.iter().any(|file| file.name.ends_with(".safetensors")) {
        return Err(not_mlx("it has no .safetensors weights"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(files: &[RemoteFile]) -> Vec<&str> {
        files.iter().map(|file| file.name.as_str()).collect()
    }

    #[test]
    fn the_listing_gives_each_files_name_and_size() {
        let json = r#"{"id":"mlx-community/X","siblings":[
            {"rfilename":"config.json","size":812},{"rfilename":"README.md"},{"size":3}]}"#;
        let files = parse_listing(json).expect("listing");
        assert_eq!(files, [RemoteFile::new("config.json", Some(812)), RemoteFile::new("README.md", None)]);
        assert!(parse_listing(r#"{"error":"nope"}"#).is_err());
        assert!(parse_listing("<html>").is_err());
    }

    #[test]
    fn only_what_mlx_lm_reads_is_downloaded() {
        let listing: Vec<RemoteFile> = [
            ".gitattributes",
            "README.md",
            "config.json",
            "generation_config.json",
            "model-00001-of-00002.safetensors",
            "model-00002-of-00002.safetensors",
            "model.safetensors.index.json",
            "special_tokens_map.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "tokenizer.model",
            "merges.txt",
            "chat_template.jinja",
            "figures/benchmark.png",
            "logo.png",
            "tokenization_custom.py",
            "../escape.json",
            "sub/config.json",
        ]
        .into_iter()
        .map(|name| RemoteFile::new(name, None))
        .collect();
        assert_eq!(
            names(&wanted(&listing)),
            [
                "config.json",
                "generation_config.json",
                "model-00001-of-00002.safetensors",
                "model-00002-of-00002.safetensors",
                "model.safetensors.index.json",
                "special_tokens_map.json",
                "tokenizer.json",
                "tokenizer_config.json",
                "tokenizer.model",
                "merges.txt",
                "chat_template.jinja",
            ]
        );
    }

    #[test]
    fn a_repository_without_a_config_or_weights_is_not_an_mlx_model() {
        let id = RepoId::parse("org/name").expect("id");
        let config = RemoteFile::new("config.json", None);
        let weights = RemoteFile::new("model.safetensors", None);
        assert!(check(&id, &[config.clone(), weights.clone()]).is_ok());
        let no_weights = check(&id, std::slice::from_ref(&config)).expect_err("no weights");
        assert!(no_weights.to_string().contains("safetensors"), "{no_weights}");
        let no_config = check(&id, &[weights]).expect_err("no config");
        assert!(no_config.to_string().contains("config.json"), "{no_config}");
    }
}
