use code_map::{FileSymbols, SymbolKind, extract};

fn kinds(file: &FileSymbols) -> Vec<(&str, SymbolKind, Option<&str>, u32)> {
    file.symbols.iter().map(|s| (s.name.as_str(), s.kind, s.container.as_deref(), s.line)).collect()
}

fn calls(file: &FileSymbols) -> Vec<(&str, Option<&str>)> {
    file.calls.iter().map(|call| (call.callee.as_str(), call.caller.as_deref())).collect()
}

#[test]
fn python_classes_methods_functions_and_calls() {
    let source = r#"
import os

class Indexer:
    @staticmethod
    def build(root):
        return scan_tree(os.path.join(root, "src"))

def scan_tree(path):
    def visit(node):
        print(node)
    return Indexer.build(path)

scan_tree(".")
"#;
    let file = extract("tools/index.py", source).unwrap_or_default();
    assert_eq!(
        kinds(&file),
        vec![
            ("Indexer", SymbolKind::Type, None, 4),
            ("build", SymbolKind::Method, Some("Indexer"), 6),
            ("scan_tree", SymbolKind::Function, None, 9),
            ("visit", SymbolKind::Function, Some("scan_tree"), 10),
        ]
    );
    let calls = calls(&file);
    assert!(calls.contains(&("scan_tree", Some("build"))));
    assert!(calls.contains(&("join", Some("build"))));
    assert!(calls.contains(&("print", Some("visit"))));
    assert!(calls.contains(&("build", Some("scan_tree"))));
    assert!(calls.contains(&("scan_tree", None)));
}

#[test]
fn go_types_methods_on_receivers_and_calls() {
    let source = r#"package store

const Limit = 10

type Reader interface {
	Read(id string) error
}

type Cache struct {
	items map[string]string
}

func (c *Cache) Get(id string) string {
	return c.load(id)
}

func NewCache() *Cache {
	fmt.Println("new")
	return &Cache{}
}
"#;
    let file = extract("store/cache.go", source).unwrap_or_default();
    assert_eq!(
        kinds(&file),
        vec![
            ("Limit", SymbolKind::Const, None, 3),
            ("Reader", SymbolKind::Interface, None, 5),
            ("Cache", SymbolKind::Type, None, 9),
            ("Get", SymbolKind::Method, Some("Cache"), 13),
            ("NewCache", SymbolKind::Function, None, 17),
        ]
    );
    let calls = calls(&file);
    assert!(calls.contains(&("load", Some("Get"))));
    assert!(calls.contains(&("Println", Some("NewCache"))));
}
