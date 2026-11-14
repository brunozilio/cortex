use code_map::{FileSymbols, SymbolKind, extract};

const SOURCE: &str = r#"
mod store {
    pub const LIMIT: usize = 3;
}

pub struct Memory {
    items: Vec<String>,
}

pub trait Recall {
    fn recall(&self) -> usize;
}

impl<T> Recall for Wrapper<T> {
    fn recall(&self) -> usize {
        self.inner.count()
    }
}

impl Memory {
    pub fn new() -> Self {
        Memory { items: Vec::new() }
    }

    pub fn merge_near_duplicates(&mut self) {
        let other = Memory::new();
        self.items.dedup();
        helpers::compact(&other);
        println!("done");
    }
}

fn top_level() {
    Vec::<u8>::with_capacity(3);
}
"#;

fn parsed() -> FileSymbols {
    extract("src/memory.rs", SOURCE).unwrap_or_default()
}

fn find<'a>(file: &'a FileSymbols, name: &str, kind: SymbolKind) -> Option<&'a code_map::Symbol> {
    file.symbols.iter().find(|symbol| symbol.name == name && symbol.kind == kind)
}

#[test]
fn finds_items_with_kinds_lines_and_containers() {
    let file = parsed();
    assert_eq!(file.path, "src/memory.rs");
    assert_eq!(find(&file, "store", SymbolKind::Module).map(|s| s.line), Some(2));
    assert_eq!(find(&file, "LIMIT", SymbolKind::Const).and_then(|s| s.container.as_deref()), Some("store"));
    assert_eq!(find(&file, "Memory", SymbolKind::Type).map(|s| (s.line, s.end_line)), Some((6, 8)));
    assert!(find(&file, "Recall", SymbolKind::Interface).is_some());
    let merge = find(&file, "merge_near_duplicates", SymbolKind::Method);
    assert_eq!(merge.map(|s| (s.line, s.container.as_deref())), Some((25, Some("Memory"))));
    assert_eq!(find(&file, "top_level", SymbolKind::Function).and_then(|s| s.container.clone()), None);
}

#[test]
fn trait_methods_and_generic_impls_are_methods_of_their_type() {
    let file = parsed();
    let containers: Vec<Option<&str>> = file
        .symbols
        .iter()
        .filter(|symbol| symbol.name == "recall")
        .map(|symbol| {
            assert_eq!(symbol.kind, SymbolKind::Method);
            symbol.container.as_deref()
        })
        .collect();
    assert_eq!(containers, vec![Some("Recall"), Some("Wrapper")]);
}

#[test]
fn calls_name_the_bare_callee_its_path_and_the_enclosing_function() {
    let file = parsed();
    let calls: Vec<(&str, Option<&str>, Option<&str>)> = file
        .calls
        .iter()
        .map(|call| (call.callee.as_str(), call.qualified.as_deref(), call.caller.as_deref()))
        .collect();
    assert!(calls.contains(&("count", None, Some("recall"))));
    assert!(calls.contains(&("new", Some("Memory::new"), Some("merge_near_duplicates"))));
    assert!(calls.contains(&("dedup", None, Some("merge_near_duplicates"))));
    assert!(calls.contains(&("compact", Some("helpers::compact"), Some("merge_near_duplicates"))));
    assert!(calls.contains(&("println", None, Some("merge_near_duplicates"))));
    assert!(calls.contains(&("with_capacity", Some("Vec::<u8>::with_capacity"), Some("top_level"))));
    let line = file.calls.iter().find(|call| call.callee == "dedup").map(|call| call.line);
    assert_eq!(line, Some(27));
}

#[test]
fn unsupported_extensions_are_not_extracted() {
    assert_eq!(extract("README.md", "# hi"), None);
    assert_eq!(extract("Makefile", "all:"), None);
}
