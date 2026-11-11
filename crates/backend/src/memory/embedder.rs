//! Text to vectors for semantic memory search. The model runs on this
//! machine (ONNX through fastembed, feature `embeddings`); it loads on a
//! background thread when first needed, downloading on first use, and searches fall back to
//! words alone until it is ready or when there is none.

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Query,
    Passage,
}

pub trait Embedder: Send + Sync {
    fn model(&self) -> &str;
    fn embed(&self, texts: &[String], purpose: Purpose) -> Option<Vec<Vec<f32>>>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoEmbedder;

impl Embedder for NoEmbedder {
    fn model(&self) -> &str {
        "none"
    }

    fn embed(&self, _texts: &[String], _purpose: Purpose) -> Option<Vec<Vec<f32>>> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelChoice {
    MiniLm,
    Mpnet,
    Off,
}

impl ModelChoice {
    pub fn from_env() -> Self {
        Self::parse(std::env::var("SUPERFIK_MEMORY_MODEL").ok().as_deref())
    }

    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("mpnet") => Self::Mpnet,
            Some("off" | "none" | "0") => Self::Off,
            _ => Self::MiniLm,
        }
    }
}

pub fn local(choice: ModelChoice, cache: PathBuf) -> Arc<dyn Embedder> {
    match choice {
        ModelChoice::Off => Arc::new(NoEmbedder),
        #[cfg(feature = "embeddings")]
        choice => Arc::new(fast::FastEmbedder::start(choice, cache)),
        #[cfg(not(feature = "embeddings"))]
        _ => {
            let _ = cache;
            Arc::new(NoEmbedder)
        }
    }
}

#[cfg(feature = "embeddings")]
mod fast {
    use std::sync::Mutex;

    use super::*;
    use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

    pub struct FastEmbedder {
        name: &'static str,
        kind: EmbeddingModel,
        cache: PathBuf,
        model: Arc<Mutex<Option<TextEmbedding>>>,
        loading: std::sync::Once,
    }

    impl FastEmbedder {
        pub fn start(choice: ModelChoice, cache: PathBuf) -> Self {
            let (name, kind) = match choice {
                ModelChoice::Mpnet => {
                    ("paraphrase-multilingual-mpnet-base-v2", EmbeddingModel::ParaphraseMLMpnetBaseV2)
                }
                _ => ("paraphrase-multilingual-minilm-l12-v2-q", EmbeddingModel::ParaphraseMLMiniLML12V2Q),
            };
            Self { name, kind, cache, model: Arc::new(Mutex::new(None)), loading: std::sync::Once::new() }
        }

        fn load(&self) {
            let (slot, kind, cache, name) = (Arc::clone(&self.model), self.kind.clone(), self.cache.clone(), self.name);
            let spawned = std::thread::Builder::new().name("superfik-memory-model".into()).spawn(move || {
                let options = TextInitOptions::new(kind).with_cache_dir(cache).with_show_download_progress(false);
                match TextEmbedding::try_new(options) {
                    Ok(loaded) => {
                        *slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(loaded);
                        tracing::info!(model = name, "memory model ready");
                    }
                    Err(error) => tracing::warn!(%error, "memory model unavailable; searching by words"),
                }
            });
            if let Err(error) = spawned {
                tracing::warn!(%error, "could not start the memory model");
            }
        }
    }

    impl Embedder for FastEmbedder {
        fn model(&self) -> &str {
            self.name
        }

        fn embed(&self, texts: &[String], _purpose: Purpose) -> Option<Vec<Vec<f32>>> {
            self.loading.call_once(|| self.load());
            let mut model = self.model.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let model = model.as_mut()?;
            match model.embed(texts, None) {
                Ok(vectors) => Some(vectors),
                Err(error) => {
                    tracing::warn!(%error, "memory embedding failed");
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_is_chosen_by_name() {
        assert_eq!(ModelChoice::parse(None), ModelChoice::MiniLm);
        assert_eq!(ModelChoice::parse(Some(" MPNET ")), ModelChoice::Mpnet);
        assert_eq!(ModelChoice::parse(Some("off")), ModelChoice::Off);
        assert!(NoEmbedder.embed(&["x".into()], Purpose::Query).is_none());
    }
}
