//! Text to vectors for semantic memory search. No model runs on this
//! machine: memory is searched by words ([`NoEmbedder`]). The seam stays so
//! a remote embedding service can plug in later.

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
