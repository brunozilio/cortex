use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

pub const MOST_SYMBOLS: usize = 3;
const MOST_FILES: usize = 8;
const MOST_PLACES_PER_FILE: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub name: String,
    pub line: u32,
    pub character: u32,
}

pub fn changed_lines(before: &str, after: &str) -> Option<(u32, u32)> {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let same_start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    if same_start == old.len() && same_start == new.len() {
        return None;
    }
    let same_end = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(old.len().min(new.len()) - same_start)
        .take_while(|(a, b)| a == b)
        .count();
    let first = u32::try_from(same_start).ok()?;
    let last = u32::try_from(new.len().saturating_sub(same_end).max(same_start + 1) - 1).ok()?;
    Some((first, last))
}

const SYMBOL_KINDS: &[u64] = &[5, 6, 9, 10, 11, 12, 14, 22, 23];

pub fn symbols_request(uri: &str) -> (&'static str, Value) {
    ("textDocument/documentSymbol", json!({ "textDocument": { "uri": uri } }))
}

pub fn references_request(uri: &str, symbol: &Changed) -> (&'static str, Value) {
    (
        "textDocument/references",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": symbol.line, "character": symbol.character },
            "context": { "includeDeclaration": false }
        }),
    )
}

pub fn changed_symbols(answer: &Value, (first, last): (u32, u32)) -> Vec<Changed> {
    let mut found = Vec::new();
    for item in answer.as_array().into_iter().flatten() {
        collect(item, (first, last), &mut found);
    }
    found.truncate(MOST_SYMBOLS);
    found
}

fn collect(item: &Value, edited: (u32, u32), found: &mut Vec<Changed>) {
    let range = item.get("range").unwrap_or(&item["location"]["range"]);
    let (Some(start), Some(end)) = (line(&range["start"]), line(&range["end"])) else { return };
    if end < edited.0 || start > edited.1 {
        return;
    }
    let inner_before = found.len();
    for child in item["children"].as_array().into_iter().flatten() {
        collect(child, edited, found);
    }
    let kind = item["kind"].as_u64().unwrap_or_default();
    if found.len() == inner_before && SYMBOL_KINDS.contains(&kind) {
        let at = item.get("selectionRange").unwrap_or(range);
        let (Some(name), Some(line), Some(character)) =
            (item["name"].as_str(), line(&at["start"]), at["start"]["character"].as_u64())
        else {
            return;
        };
        let character = u32::try_from(character).unwrap_or_default();
        if !found.iter().any(|known: &Changed| known.name == name) {
            found.push(Changed { name: name.to_owned(), line, character });
        }
    }
}

fn line(position: &Value) -> Option<u32> {
    u32::try_from(position["line"].as_u64()?).ok()
}

pub fn report(file: &Path, cwd: &Path, answers: &[(Changed, Value)]) -> Option<String> {
    let shown = |path: &Path| path.strip_prefix(cwd).unwrap_or(path).display().to_string();
    let mut lines = Vec::new();
    for (symbol, answer) in answers {
        let mut by_file: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for place in answer.as_array().into_iter().flatten() {
            let Some(path) = place["uri"].as_str().and_then(super::uri::path_of) else { continue };
            let Some(at) = line(&place["range"]["start"]) else { continue };
            if path != file {
                by_file.entry(shown(&path)).or_default().push(at + 1);
            }
        }
        if by_file.is_empty() {
            continue;
        }
        let total: usize = by_file.values().map(Vec::len).sum();
        let mut places: Vec<String> = by_file
            .iter()
            .take(MOST_FILES)
            .map(|(path, at)| {
                let lines: Vec<String> = at.iter().take(MOST_PLACES_PER_FILE).map(u32::to_string).collect();
                format!("{path}:{}", lines.join(","))
            })
            .collect();
        if by_file.len() > MOST_FILES {
            places.push(format!("and {} more files", by_file.len() - MOST_FILES));
        }
        let noun = if total == 1 { "place" } else { "places" };
        lines.push(format!("- {} ({total} {noun}): {}", symbol.name, places.join(", ")));
    }
    (!lines.is_empty()).then(|| {
        format!(
            "<callers>Other files use what this edit changed in {}; check they still fit before you finish:\n{}\n</callers>",
            shown(file),
            lines.join("\n")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: u32, end: u32) -> Value {
        json!({ "start": { "line": start, "character": 0 }, "end": { "line": end, "character": 1 } })
    }

    #[test]
    fn the_edited_lines_are_those_between_the_unchanged_head_and_tail() {
        assert_eq!(changed_lines("a\nb\nc\n", "a\nB\nc\n"), Some((1, 1)));
        assert_eq!(changed_lines("a\nc\n", "a\nb1\nb2\nc\n"), Some((1, 2)));
        assert_eq!(changed_lines("a\nb\nc\n", "a\nc\n"), Some((1, 1)));
        assert_eq!(changed_lines("", "x\n"), Some((0, 0)));
        assert_eq!(changed_lines("same\n", "same\n"), None);
    }

    #[test]
    fn the_innermost_symbols_around_the_edit_are_the_changed_ones() {
        let symbols = json!([
            { "name": "Client", "kind": 23, "range": range(0, 30), "selectionRange": range(0, 0),
              "children": [
                { "name": "retry", "kind": 6, "range": range(4, 10), "selectionRange": { "start": { "line": 4, "character": 7 }, "end": { "line": 4, "character": 12 } } },
                { "name": "close", "kind": 6, "range": range(12, 14), "selectionRange": range(12, 12) }
              ] },
            { "name": "helper", "kind": 12, "range": range(40, 44), "selectionRange": range(40, 40) },
            { "name": "LIMIT", "kind": 14, "location": { "uri": "file:///w/a.rs", "range": range(50, 50) } }
        ]);

        assert_eq!(changed_symbols(&symbols, (6, 6)), [Changed { name: "retry".into(), line: 4, character: 7 }]);
        assert_eq!(changed_symbols(&symbols, (20, 20)).iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Client"]);
        assert_eq!(changed_symbols(&symbols, (50, 50))[0].name, "LIMIT");
        assert!(changed_symbols(&symbols, (60, 61)).is_empty());
        assert_eq!(changed_symbols(&json!(null), (0, 0)), []);
    }

    #[test]
    fn the_callers_outside_the_edited_file_are_listed_by_file() {
        let retry = Changed { name: "retry".into(), line: 4, character: 7 };
        let unused = Changed { name: "close".into(), line: 12, character: 0 };
        let places = json!([
            { "uri": "file:///w/src/a.rs", "range": range(9, 9) },
            { "uri": "file:///w/src/a.rs", "range": range(19, 19) },
            { "uri": "file:///w/src/b.rs", "range": range(2, 2) },
            { "uri": "file:///w/src/client.rs", "range": range(30, 30) }
        ]);
        let only_itself = json!([{ "uri": "file:///w/src/client.rs", "range": range(1, 1) }]);

        let block = report(Path::new("/w/src/client.rs"), Path::new("/w"), &[(retry, places), (unused, only_itself)])
            .expect("report");

        assert_eq!(
            block,
            "<callers>Other files use what this edit changed in src/client.rs; check they still fit before you \
             finish:\n- retry (3 places): src/a.rs:10,20, src/b.rs:3\n</callers>"
        );
        assert_eq!(report(Path::new("/w/x.rs"), Path::new("/w"), &[]), None);
    }

    #[test]
    fn the_requests_ask_for_the_file_s_symbols_and_the_references_without_the_declaration() {
        let symbol = Changed { name: "f".into(), line: 3, character: 4 };
        assert_eq!(symbols_request("file:///a.rs").0, "textDocument/documentSymbol");
        let (method, params) = references_request("file:///a.rs", &symbol);
        assert_eq!(method, "textDocument/references");
        assert_eq!(params["position"], json!({ "line": 3, "character": 4 }));
        assert_eq!(params["context"]["includeDeclaration"], json!(false));
    }
}
