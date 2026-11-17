mod rank;
mod render;
mod words;

use serde::{Deserialize, Serialize};

use crate::extract::{FileSymbols, SymbolKind};

pub use render::render;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeMap {
    pub files: Vec<FileSymbols>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolHit {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub callers: usize,
    pub caller_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointer {
    pub path: String,
    pub symbols: Vec<SymbolHit>,
}

impl CodeMap {
    pub fn new(files: Vec<FileSymbols>) -> Self {
        CodeMap { files }
    }

    pub fn relevant(&self, prompt: &str, limit: usize) -> Vec<Pointer> {
        rank::relevant(self, prompt, limit)
    }
}
