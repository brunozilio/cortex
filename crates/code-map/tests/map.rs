use code_map::{CodeMap, Pointer, SymbolHit, SymbolKind, extract, render};

fn file(path: &str, source: &str) -> code_map::FileSymbols {
    extract(path, source).unwrap_or_default()
}

fn sample() -> CodeMap {
    CodeMap::new(vec![
        file(
            "crates/domain/src/memory/dedup.rs",
            "pub fn merge_near_duplicates(items: &mut Vec<String>) {\n    items.sort();\n}\n\npub fn similarity() -> f32 {\n    0.0\n}\n",
        ),
        file(
            "crates/backend/src/memory.rs",
            "fn save() {\n    dedup::merge_near_duplicates(&mut v);\n}\n\nfn load() {\n    merge_near_duplicates(&mut v);\n    merge_near_duplicates(&mut w);\n}\n",
        ),
        file("crates/merge/src/near.rs", "fn unrelated() {}\n"),
        file("crates/cli/src/render.rs", "fn draw_status_line() {}\n"),
    ])
}

#[test]
fn an_exact_identifier_outranks_path_words() {
    let pointers = sample().relevant("why does merge_near_duplicates drop near items when merging?", 5);
    let first = pointers.first().cloned();
    assert_eq!(
        first,
        Some(Pointer {
            path: "crates/domain/src/memory/dedup.rs".into(),
            symbols: vec![SymbolHit {
                name: "merge_near_duplicates".into(),
                kind: SymbolKind::Function,
                line: 1,
                callers: 3,
                caller_files: vec!["crates/backend/src/memory.rs".into()],
            }],
        })
    );
    let paths: Vec<&str> = pointers.iter().map(|p| p.path.as_str()).collect();
    assert!(paths.iter().position(|p| *p == "crates/merge/src/near.rs") > Some(0));
}

#[test]
fn camel_case_prompts_match_snake_case_symbols_and_stop_words_match_nothing() {
    let map = sample();
    let pointers = map.relevant("the drawStatusLine function", 5);
    assert_eq!(pointers.first().map(|p| p.path.as_str()), Some("crates/cli/src/render.rs"));
    assert!(map.relevant("please fix the code and make it work with this file", 5).is_empty());
    assert!(map.relevant("", 5).is_empty());
}

#[test]
fn a_mentioned_path_is_pointed_to() {
    let pointers = sample().relevant("look at near.rs", 5);
    assert_eq!(pointers.first().map(|p| p.path.as_str()), Some("crates/merge/src/near.rs"));
}

#[test]
fn a_name_defined_in_many_files_counts_less_than_a_rare_one() {
    let mut files: Vec<code_map::FileSymbols> = (0..8)
        .map(|i| file(&format!("src/guard{i}.rs"), "impl Drop for G {\n    fn drop(&mut self) {}\n}\n"))
        .collect();
    files.push(file("src/memories.rs", "pub struct Memories;\n"));
    let pointers = CodeMap::new(files).relevant("why does it drop memories", 5);
    let paths: Vec<&str> = pointers.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(paths, vec!["src/memories.rs"]);
}

#[test]
fn relevant_respects_the_limit() {
    assert_eq!(sample().relevant("merge near duplicates memory similarity", 1).len(), 1);
}

fn hit(name: &str, callers: usize, files: &[&str]) -> SymbolHit {
    SymbolHit {
        name: name.into(),
        kind: SymbolKind::Function,
        line: 42,
        callers,
        caller_files: files.iter().map(|f| (*f).into()).collect(),
    }
}

#[test]
fn render_shapes_a_compact_block() {
    let pointers = vec![
        Pointer { path: "crates/x/src/a.rs".into(), symbols: vec![hit("merge", 3, &["crates/y/src/b.rs", "c.rs"])] },
        Pointer { path: "crates/x/src/b.rs".into(), symbols: vec![hit("solo", 1, &["d.rs"]), hit("idle", 0, &[])] },
    ];
    assert_eq!(
        render(&pointers),
        "<code-map>\nFiles and symbols that match this prompt (from a tree-sitter index; open them rather than searching):\n\
         - crates/x/src/a.rs: fn merge (L42, called from 3 places: crates/y/src/b.rs, c.rs)\n\
         - crates/x/src/b.rs: fn solo (L42, called from 1 place: d.rs), fn idle (L42)\n</code-map>"
    );
    assert_eq!(render(&[]), "");
}

#[test]
fn render_caps_files_symbols_and_caller_files() {
    let symbols: Vec<SymbolHit> =
        (0..6).map(|i| hit(&format!("f{i}"), 9, &["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"])).collect();
    let pointers: Vec<Pointer> =
        (0..20).map(|i| Pointer { path: format!("p{i}.rs"), symbols: symbols.clone() }).collect();
    let block = render(&pointers);
    let lines: Vec<&str> = block.lines().filter(|line| line.starts_with("- ")).collect();
    assert_eq!(lines.len(), 12);
    assert_eq!(lines[0].matches("fn f").count(), 4);
    assert!(lines[0].contains("called from 9 places: a.rs, b.rs, c.rs, ...)"));
}
