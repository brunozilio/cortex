use super::*;

pub(super) fn by_agent(now: i64) -> Provenance {
    Provenance { author: Author::Agent, now, origin: Some("Night sky".into()) }
}

pub(super) fn remember(kind: MemoryKind, text: &str, tags: &[&str]) -> MemoryEdit {
    MemoryEdit::Remember { kind, text: text.into(), tags: tags.iter().map(|t| (*t).to_owned()).collect() }
}

pub(super) fn book() -> MemoryBook {
    let mut book = MemoryBook::default();
    book.apply(
        &[
            remember(MemoryKind::Semantic, "The renderer draws every surface as a signed distance field", &["render"]),
            remember(MemoryKind::Procedural, "Run cargo clippy with -D warnings before committing", &["ci"]),
            remember(MemoryKind::Episodic, "Added the context node and its usage grid", &["render", "context"]),
            remember(MemoryKind::Semantic, "The user writes in Portuguese", &["user"]),
        ],
        &by_agent(100),
    );
    book
}

#[test]
fn remembering_numbers_cleans_and_skips_repeats() {
    let mut book = book();
    assert_eq!(book.memories.iter().map(|m| m.id.0).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    let learned = book.apply(
        &[
            remember(MemoryKind::Semantic, "  the user   WRITES in portuguese ", &[]),
            remember(MemoryKind::Episodic, "   ", &[]),
            remember(MemoryKind::Semantic, "Prefers small commits", &[" #Git ", "git", ""]),
        ],
        &by_agent(200),
    );
    assert_eq!(learned, vec![MemoryId(4)]);
    let added = &book.memories[4];
    assert_eq!(
        (added.text.as_str(), added.tags.clone(), added.created),
        ("Prefers small commits", vec!["git".to_owned()], 200)
    );
    assert_eq!(added.origin.as_deref(), Some("Night sky"));
}

#[test]
fn ids_are_never_reused_after_forgetting() {
    let mut book = book();
    book.apply(&[MemoryEdit::Forget { id: MemoryId(3) }], &by_agent(1));
    let learned = book.apply(&[remember(MemoryKind::Semantic, "New fact", &[])], &by_agent(2));
    assert_eq!(learned, vec![MemoryId(4)]);
}

#[test]
fn revising_changes_text_and_ignores_unknown_ids() {
    let mut book = book();
    let touched = book.apply(
        &[
            MemoryEdit::Revise { id: MemoryId(1), text: "Run clippy and tests".into() },
            MemoryEdit::Revise { id: MemoryId(99), text: "x".into() },
        ],
        &by_agent(1),
    );
    assert_eq!(touched, vec![MemoryId(1)]);
    assert_eq!(book.memories[1].text, "Run clippy and tests");
}

#[test]
fn long_texts_are_cut_at_a_word() {
    let mut book = MemoryBook::default();
    book.apply(&[remember(MemoryKind::Semantic, &"word ".repeat(400), &[])], &by_agent(1));
    let text = &book.memories[0].text;
    assert!(text.ends_with("word…") && text.chars().count() <= MAX_MEMORY_CHARS + 1);
}

#[test]
fn shared_tags_link_memories_in_a_chain() {
    let mut memories = book().memories;
    memories[1].tags.push("render".into());
    assert_eq!(links(&memories), vec![(0, 1), (1, 2)]);
}

#[test]
fn activation_matches_prefixes_and_spreads_along_links() {
    let memories = book().memories;
    assert!(activate("", &memories).iter().all(|s| *s == 0.0));
    let scores = activate("signed dist", &memories);
    assert_eq!(scores[0], 1.0);
    assert!(scores[2] > 0.0 && scores[2] <= 0.5, "linked by the render tag: {scores:?}");
    assert_eq!((scores[1], scores[3]), (0.0, 0.0));
    let tagged = activate("ci", &memories);
    assert_eq!(tagged[1], 1.0);
}

#[test]
fn recall_keeps_facts_and_the_best_episodes() {
    let mut book = book();
    for n in 0..5 {
        book.apply(
            &[remember(MemoryKind::Episodic, &format!("Episode number {n} about sounds"), &[])],
            &by_agent(300 + n),
        );
    }
    let recalled = recall("usage grid", &book.memories, None, RecallBudget { facts: 2, episodes: 2 }, 400);
    let ids: Vec<u32> = recalled.iter().map(|m| m.id.0).collect();
    assert_eq!(recalled.iter().filter(|m| m.kind == MemoryKind::Episodic).count(), 2);
    assert!(ids.contains(&2) && ids.contains(&8), "{ids:?}");
    assert_eq!(recalled.len(), 4);
}

#[test]
fn instructions_list_memories_only_on_a_fresh_session() {
    let brief = MemoryBrief { recalled: book().memories, output: "/tmp/m.json".into(), ..MemoryBrief::default() };
    let fresh = brief.instructions(true).expect("text");
    assert!(fresh.contains("Facts:\n- [0] The renderer") && fresh.contains("Episodes:\n- [2]"));
    assert!(fresh.contains("/tmp/m.json"));
    let resumed = brief.instructions(false).expect("text");
    assert!(!resumed.contains("[0]") && resumed.contains("/tmp/m.json"));
    assert_eq!(MemoryBrief::new(None).instructions(true), None);
}

#[test]
fn edit_files_parse_strictly() {
    let json = r#"{"remember":[{"kind":"Semantic","text":"Uses wgpu","tags":["render"]},{"kind":"mood","text":"x"},{"kind":"episodic","text":" "}],"forget":[3],"revise":[{"id":1,"text":"New"},{"id":2,"text":""}]}"#;
    let edits = parse_memory_edits(json).expect("edits");
    assert_eq!(
        edits,
        vec![
            MemoryEdit::Revise { id: MemoryId(1), text: "New".into() },
            MemoryEdit::Forget { id: MemoryId(3) },
            MemoryEdit::Remember { kind: MemoryKind::Semantic, text: "Uses wgpu".into(), tags: vec!["render".into()] },
        ]
    );
    assert_eq!(parse_memory_edits("{}"), Some(vec![]));
    assert_eq!(parse_memory_edits("nope"), None);
}

fn vectors_from(pairs: &[(f32, f32)]) -> Vec<Vec<f32>> {
    pairs.iter().map(|(x, y)| vec![*x, *y]).collect()
}

#[test]
fn bm25_prefers_rare_words_and_whole_short_ones() {
    let memories = book().memories;
    let scores = search::lexical("clippy renderer", &memories);
    assert!(scores[1] > 0.0 && scores[0] > 0.0);
    assert_eq!(scores[3], 0.0);
    assert!(search::lexical("ci", &memories)[1] > 0.0);
    assert_eq!(search::lexical("re", &memories)[0], 0.0, "two letters never match a prefix");
}

#[test]
fn similarity_keeps_the_related_few() {
    let vectors = vectors_from(&[(1.0, 0.0), (0.9, 0.1), (0.0, 1.0), (-1.0, 0.0)]);
    let aligned: Vec<Option<&[f32]>> = vectors.iter().map(|v| Some(v.as_slice())).chain([None]).collect();
    let scores = search::similarity(Semantic { query: &[1.0, 0.0], memories: &aligned });
    assert!(scores[0] > scores[1] && scores[1] > 0.0);
    assert_eq!((scores[2], scores[3], scores[4]), (0.0, 0.0, 0.0));
}

#[test]
fn meaning_finds_what_words_miss() {
    let memories = book().memories;
    let vectors = vectors_from(&[(1.0, 0.0), (0.0, 1.0), (0.2, 0.8), (0.0, -1.0)]);
    let aligned: Vec<Option<&[f32]>> = vectors.iter().map(|v| Some(v.as_slice())).collect();
    let semantic = Semantic { query: &[0.95, 0.05], memories: &aligned };
    assert!(activate("desenho de superfícies", &memories).iter().all(|s| *s == 0.0));
    let scores = search("desenho de superfícies", &memories, Some(semantic));
    assert_eq!(scores[0], 1.0);
    assert!(scores[3] == 0.0);
    let both = search("clippy", &memories, Some(Semantic { query: &[0.0, 1.0], memories: &aligned }));
    assert_eq!(both[1], 1.0);
}

#[test]
fn vectors_follow_the_model_and_the_text() {
    let mut book = book();
    assert_eq!(book.unembedded("mini").len(), 4);
    book.vectors.insert(MemoryId(0), vec![1.0]);
    book.vectors.insert(MemoryId(1), vec![1.0]);
    assert_eq!(book.unembedded("mini").len(), 2);
    book.apply(
        &[MemoryEdit::Revise { id: MemoryId(0), text: "Changed".into() }, MemoryEdit::Forget { id: MemoryId(1) }],
        &by_agent(1),
    );
    assert!(book.vectors.is_empty(), "revised and forgotten memories lose their vectors");
    book.vectors.insert(MemoryId(2), vec![1.0]);
    assert_eq!(book.unembedded("other").len(), 3, "another model starts over");
    assert!(book.vectors.is_empty());
}

#[test]
fn a_search_tool_is_announced() {
    let brief = MemoryBrief { tool: true, ..MemoryBrief::default() };
    assert!(brief.instructions(false).expect("text").contains("`memory_search` tool"));
}

#[test]
fn a_memory_can_change_kind() {
    let mut book = book();
    let touched = book.apply(
        &[
            MemoryEdit::Reclassify { id: MemoryId(0), kind: MemoryKind::Procedural },
            MemoryEdit::Reclassify { id: MemoryId(1), kind: MemoryKind::Procedural },
        ],
        &by_agent(1),
    );
    assert_eq!(touched, vec![MemoryId(0)], "an unchanged kind is no change");
    assert_eq!(book.memories[0].kind, MemoryKind::Procedural);
}

#[test]
fn edit_files_can_move_memories_to_another_kind() {
    let edits =
        parse_memory_edits(r#"{"reclassify":[{"id":2,"kind":"Procedural"},{"id":3,"kind":"mood"}]}"#).expect("edits");
    assert_eq!(edits, vec![MemoryEdit::Reclassify { id: MemoryId(2), kind: MemoryKind::Procedural }]);
}
