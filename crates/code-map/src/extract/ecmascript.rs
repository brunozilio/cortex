use tree_sitter::{Language, Node};

use super::SymbolKind;
use super::walk::{Callee, Definition, Grammar, call_through, field_text, symbol};

pub(crate) fn typescript() -> Grammar {
    Grammar { language: Language::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT), define, call }
}

pub(crate) fn tsx() -> Grammar {
    Grammar { language: Language::new(tree_sitter_typescript::LANGUAGE_TSX), define, call }
}

pub(crate) fn javascript() -> Grammar {
    Grammar { language: Language::new(tree_sitter_javascript::LANGUAGE), define, call }
}

fn define(node: Node, bytes: &[u8]) -> Option<Definition> {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" | "function_signature" => {
            symbol(node, bytes, SymbolKind::Function)
        }
        "method_definition" | "method_signature" | "abstract_method_signature" => {
            symbol(node, bytes, SymbolKind::Method)
        }
        "class_declaration" | "abstract_class_declaration" | "type_alias_declaration" | "enum_declaration" => {
            symbol(node, bytes, SymbolKind::Type)
        }
        "interface_declaration" => symbol(node, bytes, SymbolKind::Interface),
        "internal_module" => symbol(node, bytes, SymbolKind::Module),
        "variable_declarator" => function_value(node, bytes, "name", SymbolKind::Function),
        "public_field_definition" => function_value(node, bytes, "name", SymbolKind::Method),
        "field_definition" => function_value(node, bytes, "property", SymbolKind::Method),
        _ => None,
    }
}

fn function_value(node: Node, bytes: &[u8], name_field: &str, kind: SymbolKind) -> Option<Definition> {
    let value = node.child_by_field_name("value")?;
    if !matches!(value.kind(), "arrow_function" | "function_expression" | "function" | "generator_function") {
        return None;
    }
    Some(Definition::Symbol { name: field_text(node, name_field, bytes)?, kind, container: None })
}

fn call(node: Node, bytes: &[u8]) -> Option<Callee> {
    match node.kind() {
        "call_expression" => call_through(node, bytes, "function"),
        "new_expression" => call_through(node, bytes, "constructor"),
        _ => None,
    }
}
