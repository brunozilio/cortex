use tree_sitter::{Language, Node, Parser};

use super::{Call, FileSymbols, Symbol, SymbolKind};

pub(crate) enum Definition {
    Symbol { name: String, kind: SymbolKind, container: Option<String> },
    Scope { name: String },
}

pub(crate) struct Callee {
    pub name: String,
    pub qualified: Option<String>,
}

pub(crate) struct Grammar {
    pub language: Language,
    pub define: fn(Node, &[u8]) -> Option<Definition>,
    pub call: fn(Node, &[u8]) -> Option<Callee>,
}

struct Frame {
    depth: usize,
    name: String,
    kind: Option<SymbolKind>,
}

pub(crate) fn walk(grammar: &Grammar, path: &str, source: &str) -> Option<FileSymbols> {
    let mut parser = Parser::new();
    parser.set_language(&grammar.language).ok()?;
    let tree = parser.parse(source, None)?;
    let bytes = source.as_bytes();
    let mut out = FileSymbols { path: path.to_owned(), ..FileSymbols::default() };
    let mut frames: Vec<Frame> = Vec::new();
    let mut cursor = tree.walk();
    let mut depth = 0usize;
    loop {
        visit(grammar, cursor.node(), bytes, depth, &mut frames, &mut out);
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            while frames.last().is_some_and(|frame| frame.depth >= depth) {
                frames.pop();
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Some(out);
            }
            depth = depth.saturating_sub(1);
        }
    }
}

fn visit(grammar: &Grammar, node: Node, bytes: &[u8], depth: usize, frames: &mut Vec<Frame>, out: &mut FileSymbols) {
    if let Some(callee) = (grammar.call)(node, bytes) {
        let caller = frames.iter().rev().find(|frame| frame.kind.is_some()).map(|frame| frame.name.clone());
        out.calls.push(Call { callee: callee.name, qualified: callee.qualified, line: line(node), caller });
    }
    match (grammar.define)(node, bytes) {
        Some(Definition::Symbol { name, kind, container }) => {
            let enclosing = frames.last();
            let kind = match (kind, enclosing.map(|frame| frame.kind)) {
                (SymbolKind::Function, Some(None | Some(SymbolKind::Type | SymbolKind::Interface))) => {
                    SymbolKind::Method
                }
                (kind, _) => kind,
            };
            let container = container.or_else(|| enclosing.map(|frame| frame.name.clone()));
            out.symbols.push(Symbol {
                name: name.clone(),
                kind,
                line: line(node),
                end_line: u32::try_from(node.end_position().row + 1).unwrap_or(u32::MAX),
                container,
            });
            frames.push(Frame { depth, name, kind: Some(kind) });
        }
        Some(Definition::Scope { name }) => frames.push(Frame { depth, name, kind: None }),
        None => {}
    }
}

fn line(node: Node) -> u32 {
    u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX)
}

pub(crate) fn text(node: Node, bytes: &[u8]) -> Option<String> {
    node.utf8_text(bytes).ok().map(str::to_owned)
}

pub(crate) fn field_text(node: Node, field: &str, bytes: &[u8]) -> Option<String> {
    text(node.child_by_field_name(field)?, bytes)
}

pub(crate) fn symbol(node: Node, bytes: &[u8], kind: SymbolKind) -> Option<Definition> {
    Some(Definition::Symbol { name: field_text(node, "name", bytes)?, kind, container: None })
}

pub(crate) fn callee_name(node: Node, bytes: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier"
        | "field_identifier"
        | "property_identifier"
        | "private_property_identifier"
        | "type_identifier" => text(node, bytes),
        _ => ["name", "field", "property", "attribute", "function", "type"]
            .iter()
            .find_map(|field| node.child_by_field_name(field))
            .and_then(|child| callee_name(child, bytes)),
    }
}

pub(crate) fn call_through(node: Node, bytes: &[u8], field: &str) -> Option<Callee> {
    let name = callee_name(node.child_by_field_name(field)?, bytes)?;
    Some(Callee { name, qualified: None })
}

pub(crate) fn first_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find_map(|child| first_descendant(child, kind))
}
