use code_map::{FileSymbols, SymbolKind, extract};

fn kinds(file: &FileSymbols) -> Vec<(&str, SymbolKind, Option<&str>)> {
    file.symbols.iter().map(|s| (s.name.as_str(), s.kind, s.container.as_deref())).collect()
}

fn calls(file: &FileSymbols) -> Vec<(&str, Option<&str>)> {
    file.calls.iter().map(|call| (call.callee.as_str(), call.caller.as_deref())).collect()
}

const TYPESCRIPT: &str = r#"
export interface Store {
  load(id: string): Item;
}

type Id = string;

export class SessionStore implements Store {
  private cache = new Map<string, Item>();

  load(id: string): Item {
    return this.cache.get(id) ?? fetchItem(id);
  }

  handle = (event: Event) => {
    this.load(event.id);
  };
}

export function fetchItem(id: string): Item {
  return api.client.request(id);
}

const formatName = (name: string) => name.trim();
"#;

#[test]
fn typescript_symbols_and_calls() {
    let file = extract("src/store.ts", TYPESCRIPT).unwrap_or_default();
    let symbols = kinds(&file);
    assert!(symbols.contains(&("Store", SymbolKind::Interface, None)));
    assert!(symbols.contains(&("load", SymbolKind::Method, Some("Store"))));
    assert!(symbols.contains(&("Id", SymbolKind::Type, None)));
    assert!(symbols.contains(&("SessionStore", SymbolKind::Type, None)));
    assert!(symbols.contains(&("load", SymbolKind::Method, Some("SessionStore"))));
    assert!(symbols.contains(&("handle", SymbolKind::Method, Some("SessionStore"))));
    assert!(symbols.contains(&("fetchItem", SymbolKind::Function, None)));
    assert!(symbols.contains(&("formatName", SymbolKind::Function, None)));
    let calls = calls(&file);
    assert!(calls.contains(&("Map", Some("SessionStore"))));
    assert!(calls.contains(&("get", Some("load"))));
    assert!(calls.contains(&("fetchItem", Some("load"))));
    assert!(calls.contains(&("load", Some("handle"))));
    assert!(calls.contains(&("request", Some("fetchItem"))));
    assert!(calls.contains(&("trim", Some("formatName"))));
}

#[test]
fn tsx_components_are_functions() {
    let source = "export function Panel() {\n  const rows = useRows();\n  return <div>{rows.map(render)}</div>;\n}\n";
    let file = extract("src/Panel.tsx", source).unwrap_or_default();
    assert_eq!(kinds(&file), vec![("Panel", SymbolKind::Function, None)]);
    assert!(calls(&file).contains(&("useRows", Some("Panel"))));
    assert!(calls(&file).contains(&("map", Some("Panel"))));
}

#[test]
fn javascript_classes_functions_and_calls() {
    let source = r#"
class Queue {
  push(item) {
    this.items.push(item);
    notify();
  }
}

function notify() {
  console.log("pushed");
}

const drain = function () { return new Queue(); };
"#;
    let file = extract("lib/queue.mjs", source).unwrap_or_default();
    assert_eq!(
        kinds(&file),
        vec![
            ("Queue", SymbolKind::Type, None),
            ("push", SymbolKind::Method, Some("Queue")),
            ("notify", SymbolKind::Function, None),
            ("drain", SymbolKind::Function, None),
        ]
    );
    let calls = calls(&file);
    assert!(calls.contains(&("push", Some("push"))));
    assert!(calls.contains(&("notify", Some("push"))));
    assert!(calls.contains(&("log", Some("notify"))));
    assert!(calls.contains(&("Queue", Some("drain"))));
    let push = file.symbols.iter().find(|s| s.name == "push").map(|s| (s.line, s.end_line));
    assert_eq!(push, Some((3, 6)));
}
