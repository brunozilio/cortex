use tree_sitter::{Language, Node};

use super::SymbolKind;
use super::walk::{Callee, Definition, Grammar, call_through, symbol};

pub(crate) fn grammar() -> Grammar {
    Grammar { language: Language::new(tree_sitter_python::LANGUAGE), define, call }
}

fn define(node: Node, bytes: &[u8]) -> Option<Definition> {
    match node.kind() {
        "function_definition" => symbol(node, bytes, SymbolKind::Function),
        "class_definition" => symbol(node, bytes, SymbolKind::Type),
        _ => None,
    }
}

fn call(node: Node, bytes: &[u8]) -> Option<Callee> {
    match node.kind() {
        "call" => call_through(node, bytes, "function"),
        _ => None,
    }
}
