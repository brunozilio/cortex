mod ecmascript;
mod go;
mod python;
mod rust;
mod walk;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Type,
    Interface,
    Module,
    Const,
    Macro,
}

impl SymbolKind {
    pub fn label(self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Method => "method",
            SymbolKind::Type => "type",
            SymbolKind::Interface => "interface",
            SymbolKind::Module => "mod",
            SymbolKind::Const => "const",
            SymbolKind::Macro => "macro",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub end_line: u32,
    pub container: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub callee: String,
    pub qualified: Option<String>,
    pub line: u32,
    pub caller: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FileSymbols {
    pub path: String,
    pub symbols: Vec<Symbol>,
    pub calls: Vec<Call>,
}

pub fn supported(path: &str) -> bool {
    grammar(path).is_some()
}

pub fn extract(path: &str, source: &str) -> Option<FileSymbols> {
    walk::walk(&grammar(path)?, path, source)
}

fn grammar(path: &str) -> Option<walk::Grammar> {
    let extension = path.rsplit_once('.')?.1;
    match extension {
        "rs" => Some(rust::grammar()),
        "ts" | "mts" | "cts" => Some(ecmascript::typescript()),
        "tsx" => Some(ecmascript::tsx()),
        "js" | "jsx" | "mjs" | "cjs" => Some(ecmascript::javascript()),
        "py" | "pyi" => Some(python::grammar()),
        "go" => Some(go::grammar()),
        _ => None,
    }
}
