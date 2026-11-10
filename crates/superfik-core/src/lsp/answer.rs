//! A language server's answer, as short text for the agent: places as
//! `path:line:column` (from 1, relative to the working folder) with the
//! line they point at.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::Operation;

/// The answer to `operation`. Paths show relative to the first of `cwd`
/// they are in (the working folder as given and its real path, since
/// servers may answer either). `line` reads a file's line (from 0), for
/// the excerpt shown next to each place.
pub fn describe(
    operation: Operation,
    answer: &Value,
    cwd: &[&Path],
    line: impl Fn(&Path, u32) -> Option<String>,
) -> String {
    let items: Vec<&Value> = match answer {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().collect(),
        single => vec![single],
    };
    let entries: Vec<String> = match operation {
        Operation::Hover => return hover(answer).unwrap_or_else(|| "No hover information.".into()),
        Operation::Definition | Operation::Implementation | Operation::References => {
            items.iter().filter_map(|item| place(item, cwd, &line)).collect()
        }
        Operation::DocumentSymbol => items.iter().flat_map(|item| outline(item, 0, cwd)).collect(),
        Operation::WorkspaceSymbol => items.iter().filter_map(|item| symbol(item, cwd)).collect(),
        Operation::IncomingCalls => items.iter().filter_map(|call| call_item(&call["from"], cwd)).collect(),
        Operation::OutgoingCalls => items.iter().filter_map(|call| call_item(&call["to"], cwd)).collect(),
    };
    let listed = if entries.len() > MAX_ENTRIES {
        format!("{}\n… and {} more", entries[..MAX_ENTRIES].join("\n"), entries.len() - MAX_ENTRIES)
    } else {
        entries.join("\n")
    };
    let (one, many) = nouns(operation);
    let single = matches!(operation, Operation::Definition | Operation::Implementation);
    match entries.len() {
        0 if single => format!("No {one} found."),
        0 => format!("No {many} found."),
        _ if matches!(operation, Operation::DocumentSymbol | Operation::WorkspaceSymbol) => listed,
        1 if single => listed,
        1 => format!("1 {one}:\n{listed}"),
        count => format!("{count} {many}:\n{listed}"),
    }
}

const MAX_ENTRIES: usize = 100;

/// A failed question, said so the agent knows whether to ask again: a
/// server answers "content modified" (or not in time) while it indexes.
pub fn explain(error: &str) -> String {
    if error.contains("content modified") || error.contains("did not answer") {
        format!("{error}. The language server is still indexing the project; try again shortly.")
    } else {
        error.to_owned()
    }
}

fn nouns(operation: Operation) -> (&'static str, &'static str) {
    match operation {
        Operation::Definition => ("definition", "definitions"),
        Operation::Implementation => ("implementation", "implementations"),
        Operation::References => ("reference", "references"),
        Operation::IncomingCalls => ("caller", "callers"),
        Operation::OutgoingCalls => ("callee", "callees"),
        Operation::Hover | Operation::DocumentSymbol | Operation::WorkspaceSymbol => ("symbol", "symbols"),
    }
}

/// `path:line:column` from 1 (the path relative to `cwd` when inside it),
/// the file and its line from 0.
pub fn position(uri: &str, range: &Value, cwd: &[&Path]) -> Option<(String, PathBuf, u32)> {
    let path = super::uri::path_of(uri)?;
    let start = &range["start"];
    let line = u32::try_from(start["line"].as_u64()?).ok()?;
    let character = start["character"].as_u64()?;
    Some((format!("{}:{}:{}", shown(&path, cwd), line + 1, character + 1), path, line))
}

fn shown(path: &Path, cwd: &[&Path]) -> String {
    cwd.iter().find_map(|root| path.strip_prefix(root).ok()).unwrap_or(path).display().to_string()
}

fn place(item: &Value, cwd: &[&Path], line: &impl Fn(&Path, u32) -> Option<String>) -> Option<String> {
    let (uri, range) = match item.get("targetUri") {
        Some(uri) => (uri, item.get("targetSelectionRange").unwrap_or(&item["targetRange"])),
        None => (&item["uri"], &item["range"]),
    };
    let (at, path, number) = position(uri.as_str()?, range, cwd)?;
    Some(match line(&path, number) {
        Some(text) if !text.trim().is_empty() => format!("{at}: {}", text.trim()),
        _ => at,
    })
}

fn hover(answer: &Value) -> Option<String> {
    let text = |part: &Value| match part {
        Value::String(text) => Some(text.clone()),
        other => other["value"].as_str().map(str::to_owned),
    };
    let parts: Vec<String> = match &answer["contents"] {
        Value::Array(parts) => parts.iter().filter_map(text).collect(),
        contents => text(contents).into_iter().collect(),
    };
    let joined = parts.join("\n\n");
    (!joined.trim().is_empty()).then_some(joined)
}

fn outline(item: &Value, depth: usize, cwd: &[&Path]) -> Vec<String> {
    if item.get("location").is_some() {
        return symbol(item, cwd).into_iter().collect();
    }
    let range = item.get("selectionRange").unwrap_or(&item["range"]);
    let Some(line) = range["start"]["line"].as_u64() else { return Vec::new() };
    let mut lines = vec![format!("{}{} (line {})", "  ".repeat(depth), named(item), line + 1)];
    for child in item["children"].as_array().into_iter().flatten() {
        lines.extend(outline(child, depth + 1, cwd));
    }
    lines
}

fn symbol(item: &Value, cwd: &[&Path]) -> Option<String> {
    let location = &item["location"];
    let uri = location["uri"].as_str()?;
    let at = match position(uri, &location["range"], cwd) {
        Some((at, _, _)) => at,
        None => shown(&super::uri::path_of(uri)?, cwd),
    };
    Some(format!("{} {at}", named(item)))
}

fn call_item(item: &Value, cwd: &[&Path]) -> Option<String> {
    let range = item.get("selectionRange").unwrap_or(&item["range"]);
    let (at, _, _) = position(item["uri"].as_str()?, range, cwd)?;
    Some(format!("{} {at}", named(item)))
}

fn named(item: &Value) -> String {
    let name = item["name"].as_str().unwrap_or("?");
    let kind = item["kind"].as_u64().and_then(|kind| KINDS.get(usize::try_from(kind.checked_sub(1)?).ok()?));
    match kind {
        Some(kind) => format!("{kind} {name}"),
        None => name.to_owned(),
    }
}

const KINDS: [&str; 26] = [
    "file",
    "module",
    "namespace",
    "package",
    "class",
    "method",
    "property",
    "field",
    "constructor",
    "enum",
    "interface",
    "function",
    "variable",
    "constant",
    "string",
    "number",
    "boolean",
    "array",
    "object",
    "key",
    "null",
    "enum member",
    "struct",
    "event",
    "operator",
    "type parameter",
];

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn range(line: u32, character: u32) -> Value {
        json!({"start": {"line": line, "character": character}, "end": {"line": line, "character": character + 3}})
    }

    fn lines(path: &Path, line: u32) -> Option<String> {
        Some(format!("    line {line} of {}", path.file_name()?.to_str()?))
    }

    #[test]
    fn places_are_listed_relative_to_the_working_folder_with_their_line() {
        let cwd: &[&Path] = &[Path::new("/w")];
        let links = json!([{"targetUri": "file:///w/src/mod.rs", "targetRange": range(9, 0), "targetSelectionRange": range(10, 4)}]);
        assert_eq!(describe(Operation::Definition, &links, cwd, lines), "src/mod.rs:11:5: line 10 of mod.rs");
        let references = json!([
            {"uri": "file:///w/a.rs", "range": range(0, 0)},
            {"uri": "file:///elsewhere/b.rs", "range": range(4, 2)}
        ]);
        assert_eq!(
            describe(Operation::References, &references, cwd, lines),
            "2 references:\na.rs:1:1: line 0 of a.rs\n/elsewhere/b.rs:5:3: line 4 of b.rs"
        );
        let aliased: &[&Path] = &[Path::new("/var/w"), Path::new("/private/var/w")];
        let canonical = json!({"uri": "file:///private/var/w/a.rs", "range": range(0, 0)});
        assert_eq!(describe(Operation::Definition, &canonical, aliased, lines), "a.rs:1:1: line 0 of a.rs");
        assert_eq!(describe(Operation::Definition, &json!(null), cwd, lines), "No definition found.");
        assert_eq!(describe(Operation::References, &json!([]), cwd, lines), "No references found.");
    }

    #[test]
    fn hover_reads_as_its_text_and_symbols_as_an_outline() {
        let cwd: &[&Path] = &[Path::new("/w")];
        let hover = json!({"contents": {"kind": "markdown", "value": "```rust\npub struct Grep\n```"}});
        assert_eq!(describe(Operation::Hover, &hover, cwd, lines), "```rust\npub struct Grep\n```");
        let old_style = json!({"contents": [{"language": "rust", "value": "fn a()"}, "Does a."]});
        assert_eq!(describe(Operation::Hover, &old_style, cwd, lines), "fn a()\n\nDoes a.");
        let outline = json!([{
            "name": "Grep", "kind": 23, "range": range(12, 0), "selectionRange": range(12, 11),
            "children": [{"name": "run", "kind": 6, "range": range(20, 4), "selectionRange": range(20, 7)}]
        }]);
        assert_eq!(
            describe(Operation::DocumentSymbol, &outline, cwd, lines),
            "struct Grep (line 13)\n  method run (line 21)"
        );
        let found = json!([{"name": "Tool", "kind": 11, "location": {"uri": "file:///w/src/tools/mod.rs", "range": range(192, 0)}}]);
        assert_eq!(describe(Operation::WorkspaceSymbol, &found, cwd, lines), "interface Tool src/tools/mod.rs:193:1");
    }

    #[test]
    fn a_server_still_indexing_is_told_apart_from_a_failure() {
        let busy = explain("rust-analyzer: content modified");
        assert!(busy.starts_with("rust-analyzer: content modified. ") && busy.contains("still indexing"), "{busy}");
        assert!(explain("rust-analyzer did not answer in 60s").contains("try again shortly"));
        assert_eq!(explain("gopls stopped"), "gopls stopped");
    }

    #[test]
    fn calls_name_the_function_on_the_other_side() {
        let cwd: &[&Path] = &[Path::new("/w")];
        let item = |name: &str, line: u32| json!({"name": name, "kind": 12, "uri": "file:///w/a.rs", "range": range(line, 0), "selectionRange": range(line, 3)});
        let incoming = json!([{"from": item("main", 2), "fromRanges": [range(5, 4)]}]);
        assert_eq!(describe(Operation::IncomingCalls, &incoming, cwd, lines), "1 caller:\nfunction main a.rs:3:4");
        let outgoing = json!([{"to": item("helper", 30), "fromRanges": [range(6, 4)]}]);
        assert_eq!(describe(Operation::OutgoingCalls, &outgoing, cwd, lines), "1 callee:\nfunction helper a.rs:31:4");
        assert_eq!(describe(Operation::IncomingCalls, &json!([]), cwd, lines), "No callers found.");
    }
}
