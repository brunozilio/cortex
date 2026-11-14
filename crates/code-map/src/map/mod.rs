mod rank;
mod render;
mod words;

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::extract::{FileSymbols, Symbol, SymbolKind};

pub use render::render;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeMap {
    pub files: Vec<FileSymbols>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub path: String,
    pub line: u32,
    pub caller: Option<String>,
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

    pub fn callers(&self, name: &str) -> Vec<CallSite> {
        self.files
            .iter()
            .flat_map(|file| {
                file.calls
                    .iter()
                    .filter(move |call| call.callee == name || call.qualified.as_deref() == Some(name))
                    .map(move |call| CallSite { path: file.path.clone(), line: call.line, caller: call.caller.clone() })
            })
            .collect()
    }

    pub fn symbols_in(&self, path: &str, lines: RangeInclusive<u32>) -> Vec<&Symbol> {
        self.files
            .iter()
            .filter(|file| file.path == path)
            .flat_map(|file| &file.symbols)
            .filter(|symbol| symbol.line <= *lines.end() && symbol.end_line >= *lines.start())
            .collect()
    }

    pub fn relevant(&self, prompt: &str, limit: usize) -> Vec<Pointer> {
        rank::relevant(self, prompt, limit)
    }
}
