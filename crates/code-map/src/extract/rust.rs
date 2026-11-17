use tree_sitter::{Language, Node};

use super::SymbolKind;
use super::walk::{Callee, Definition, FOLLOWED_AT_MOST, Grammar, callee_name, field_text, symbol, text};

pub(crate) fn grammar() -> Grammar {
    Grammar { language: Language::new(tree_sitter_rust::LANGUAGE), define, call }
}

fn define(node: Node, bytes: &[u8]) -> Option<Definition> {
    match node.kind() {
        "function_item" | "function_signature_item" => symbol(node, bytes, SymbolKind::Function),
        "struct_item" | "enum_item" | "union_item" | "type_item" => symbol(node, bytes, SymbolKind::Type),
        "trait_item" => symbol(node, bytes, SymbolKind::Interface),
        "mod_item" => symbol(node, bytes, SymbolKind::Module),
        "const_item" | "static_item" => symbol(node, bytes, SymbolKind::Const),
        "macro_definition" => symbol(node, bytes, SymbolKind::Macro),
        "impl_item" => Some(Definition::Scope { name: type_name(node.child_by_field_name("type")?, bytes)? }),
        _ => None,
    }
}

fn type_name(node: Node, bytes: &[u8]) -> Option<String> {
    let mut node = node;
    for _ in 0..FOLLOWED_AT_MOST {
        match node.kind() {
            "generic_type" | "reference_type" => node = node.child_by_field_name("type")?,
            "scoped_type_identifier" => return field_text(node, "name", bytes),
            _ => return text(node, bytes),
        }
    }
    None
}

fn call(node: Node, bytes: &[u8]) -> Option<Callee> {
    match node.kind() {
        "call_expression" => {
            let function = node.child_by_field_name("function")?;
            let qualified = qualified(function, bytes);
            Some(Callee { name: callee_name(function, bytes)?, qualified })
        }
        "macro_invocation" => Some(Callee { name: field_text(node, "macro", bytes)?, qualified: None }),
        _ => None,
    }
}

fn qualified(function: Node, bytes: &[u8]) -> Option<String> {
    let mut function = function;
    for _ in 0..FOLLOWED_AT_MOST {
        match function.kind() {
            "scoped_identifier" => return text(function, bytes).map(|path| path.split_whitespace().collect()),
            "generic_function" => function = function.child_by_field_name("function")?,
            _ => return None,
        }
    }
    None
}
