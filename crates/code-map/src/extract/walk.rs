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

pub(crate) const FOLLOWED_AT_MOST: usize = 16;

pub(crate) fn callee_name(node: Node, bytes: &[u8]) -> Option<String> {
    let mut node = node;
    for _ in 0..FOLLOWED_AT_MOST {
        match node.kind() {
            "identifier"
            | "field_identifier"
            | "property_identifier"
            | "private_property_identifier"
            | "type_identifier" => return text(node, bytes),
            _ => {
                node = ["name", "field", "property", "attribute", "function", "type"]
                    .iter()
                    .find_map(|field| node.child_by_field_name(field))?;
            }
        }
    }
    None
}

pub(crate) fn call_through(node: Node, bytes: &[u8], field: &str) -> Option<Callee> {
    let name = callee_name(node.child_by_field_name(field)?, bytes)?;
    Some(Callee { name, qualified: None })
}

pub(crate) fn first_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    let mut depth = 0usize;
    loop {
        if cursor.node().kind() == kind {
            return Some(cursor.node());
        }
        if depth < FOLLOWED_AT_MOST && cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if depth == 0 {
                return None;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            cursor.goto_parent();
            depth -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    fn on_a_small_stack<T: Send>(work: impl FnOnce() -> T + Send) -> T {
        std::thread::scope(|scope| {
            std::thread::Builder::new().stack_size(2 << 20).spawn_scoped(scope, work).expect("thread").join()
        })
        .expect("the work ran to its end without overflowing its stack")
    }

    #[test]
    fn a_call_of_a_call_of_a_call_without_end_is_indexed_without_overflowing_the_stack() {
        let (ten_times, as_found) = ("()".repeat(200_000), "()".repeat(40_000));
        for (path, source) in [
            ("a.py", format!("f{ten_times}\n")),
            ("a.rs", format!("fn m() {{ f{ten_times}; }}\n")),
            ("a.js", format!("f{as_found};\n")),
            ("a.ts", format!("f{as_found};\n")),
            ("a.go", format!("package a\nfunc m() {{ f{as_found} }}\n")),
        ] {
            let started = Instant::now();
            let found = on_a_small_stack(|| crate::extract(path, &source));
            assert!(found.is_some(), "{path}");
            assert!(started.elapsed() < Duration::from_secs(60), "{path}: {:?}", started.elapsed());
        }
    }

    #[test]
    fn a_short_chain_of_calls_still_names_its_callee() {
        let found = crate::extract("a.py", "def m():\n    f()()()\n").expect("python");
        assert!(found.calls.iter().any(|call| call.callee == "f"), "{:?}", found.calls);
        let found = crate::extract("a.go", "package a\nfunc (s *Server) Run() { s.Go() }\n").expect("go");
        assert!(found.symbols.iter().any(|symbol| symbol.container.as_deref() == Some("Server")), "{found:?}");
    }

    #[test]
    fn a_type_wrapped_in_references_without_end_names_nothing_rather_than_overflowing() {
        let source = format!("impl {}T {{ fn a() {{}} }}\n", "&".repeat(100_000));
        let found = on_a_small_stack(|| crate::extract("a.rs", &source));
        assert!(found.is_some());
    }
}
