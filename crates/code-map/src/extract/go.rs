use tree_sitter::{Language, Node};

use super::SymbolKind;
use super::walk::{Callee, Definition, Grammar, call_through, field_text, first_descendant, symbol, text};

pub(crate) fn grammar() -> Grammar {
    Grammar { language: Language::new(tree_sitter_go::LANGUAGE), define, call }
}

fn define(node: Node, bytes: &[u8]) -> Option<Definition> {
    match node.kind() {
        "function_declaration" => symbol(node, bytes, SymbolKind::Function),
        "method_declaration" => method(node, bytes),
        "type_spec" | "type_alias" => {
            let interface = node.child_by_field_name("type").is_some_and(|ty| ty.kind() == "interface_type");
            symbol(node, bytes, if interface { SymbolKind::Interface } else { SymbolKind::Type })
        }
        "const_spec" => symbol(node, bytes, SymbolKind::Const),
        _ => None,
    }
}

fn method(node: Node, bytes: &[u8]) -> Option<Definition> {
    let receiver = node.child_by_field_name("receiver").and_then(|list| first_descendant(list, "type_identifier"));
    Some(Definition::Symbol {
        name: field_text(node, "name", bytes)?,
        kind: SymbolKind::Method,
        container: receiver.and_then(|ty| text(ty, bytes)),
    })
}

fn call(node: Node, bytes: &[u8]) -> Option<Callee> {
    match node.kind() {
        "call_expression" => call_through(node, bytes, "function"),
        _ => None,
    }
}
