//! How memories are kept clean, fade and come back.

use super::tests::{book, by_agent, remember};
use super::*;

#[test]
fn secrets_never_reach_a_memory() {
    let cases = [
        ("The key is sk-ant-api03-AbCdEf0123456789xyz", "The key is [redacted]"),
        ("Token ghp_0123456789abcdefABCDEF0123456789abcd works", "Token [redacted] works"),
        ("Slack bot (xoxb-1234-5678-abcdefgh) posts", "Slack bot ([redacted]) posts"),
        ("AWS AKIAIOSFODNN7EXAMPLE in CI", "AWS [redacted] in CI"),
        ("Send Authorization: Bearer abc.def-123", "Send Authorization: Bearer [redacted]"),
        ("Login with password=hunter2 and API_KEY=xyz", "Login with password=[redacted] and API_KEY=[redacted]"),
        ("The db password: hunter2 is old", "The db password: [redacted] is old"),
        ("JWT eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl here", "JWT [redacted] here"),
        ("Run cargo test --workspace; the task-id is sk-short", "Run cargo test --workspace; the task-id is sk-short"),
    ];
    for (written, kept) in cases {
        let mut book = MemoryBook::default();
        book.apply(&[remember(MemoryKind::Semantic, written, &[])], &by_agent(1));
        assert_eq!(book.memories[0].text, kept, "from {written:?}");
    }
    let mut book = book();
    book.apply(&[MemoryEdit::Revise { id: MemoryId(0), text: "Use token=s3cr3t".into() }], &by_agent(2));
    assert_eq!(book.memories[0].text, "Use token=[redacted]", "revisions are cleaned too");
}

#[test]
fn a_forgotten_memory_can_be_restored_until_it_expires() {
    let mut book = book();
    let day = 24 * 60 * 60;
    book.apply(&[MemoryEdit::Forget { id: MemoryId(1) }], &by_agent(1_000));
    assert!(book.memories.iter().all(|m| m.id != MemoryId(1)), "out of the live memory");
    assert_eq!(book.forgotten.iter().map(|f| (f.memory.id, f.at)).collect::<Vec<_>>(), vec![(MemoryId(1), 1_000)]);

    let touched = book.apply(&[MemoryEdit::Restore { id: MemoryId(1) }], &by_agent(2_000));
    assert_eq!(touched, vec![MemoryId(1)]);
    assert_eq!(book.memories.iter().map(|m| m.id.0).collect::<Vec<_>>(), vec![0, 1, 2, 3], "back in its place");
    assert!(book.forgotten.is_empty());

    book.apply(&[MemoryEdit::Forget { id: MemoryId(2) }], &by_agent(3_000));
    book.apply(&[], &by_agent(3_000 + FORGOTTEN_KEPT_DAYS * day));
    assert_eq!(book.forgotten.len(), 1, "kept for the whole grace period");
    book.apply(&[], &by_agent(3_001 + FORGOTTEN_KEPT_DAYS * day));
    assert!(book.forgotten.is_empty(), "gone once the grace period is over");
    let learned = book.apply(&[remember(MemoryKind::Semantic, "New fact", &[])], &by_agent(4_000));
    assert_eq!(learned, vec![MemoryId(4)], "ids are not reused");
}

#[test]
fn a_revision_keeps_the_texts_it_replaced() {
    let mut book = book();
    for (n, text) in ["one", "two", "three", "four", "five", "six", "seven"].into_iter().enumerate() {
        book.apply(&[MemoryEdit::Revise { id: MemoryId(1), text: text.into() }], &by_agent(10 + n as i64));
    }
    let memory = &book.memories[1];
    assert_eq!(memory.text, "seven");
    let kept: Vec<(&str, i64)> = memory.history.iter().map(|r| (r.text.as_str(), r.until)).collect();
    assert_eq!(kept, vec![("two", 12), ("three", 13), ("four", 14), ("five", 15), ("six", 16)], "the last five");
}

#[test]
fn a_new_memory_that_says_the_same_revises_the_old_one() {
    let mut book = book();
    let since = book.next_id();
    book.apply(
        &[
            remember(MemoryKind::Semantic, "Every surface is drawn as an SDF", &["sdf"]),
            remember(MemoryKind::Procedural, "Run clippy with -D warnings", &[]),
            remember(MemoryKind::Semantic, "The CLI uses ratatui", &[]),
        ],
        &by_agent(500),
    );
    for (id, vector) in [(0, [1.0, 0.0, 0.0]), (1, [0.0, 1.0, 0.0]), (2, [1.0, 0.0, 0.0]), (3, [0.0, 0.0, 1.0])] {
        book.vectors.insert(MemoryId(id), vector.to_vec());
    }
    for (id, vector) in [(4, [0.99, 0.05, 0.0]), (5, [1.0, 0.02, 0.0]), (6, [0.5, 0.5, 0.7])] {
        book.vectors.insert(MemoryId(id), vector.to_vec());
    }

    let merged = book.merge_near_duplicates(since, 600);

    assert_eq!(merged, vec![(MemoryId(4), MemoryId(0))], "same kind only; the episode 2 and the new ratatui stay");
    let fact = &book.memories[0];
    assert_eq!(fact.text, "Every surface is drawn as an SDF", "the newer wording wins");
    assert_eq!(
        fact.history.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        vec!["The renderer draws every surface as a signed distance field"]
    );
    assert_eq!(fact.tags, vec!["render".to_owned(), "sdf".to_owned()], "tags join");
    assert_eq!(book.memories.iter().map(|m| m.id.0).collect::<Vec<_>>(), vec![0, 1, 2, 3, 5, 6]);
    assert!(!book.vectors.contains_key(&MemoryId(4)) && book.vectors[&MemoryId(0)] == vec![0.99, 0.05, 0.0]);
}

#[test]
fn old_episodes_fade_unless_they_keep_being_used() {
    let day = 24 * 60 * 60;
    let now = 200 * day;
    let mut book = MemoryBook::default();
    book.apply(&[remember(MemoryKind::Episodic, "Fixed the shader cache", &[])], &by_agent(now - 90 * day));
    book.apply(&[remember(MemoryKind::Semantic, "The shader cache lives in target", &[])], &by_agent(now - 180 * day));
    book.apply(&[remember(MemoryKind::Episodic, "Tuned the shader cache size", &[])], &by_agent(now - day));
    let one = RecallBudget { facts: 1, episodes: 1 };
    let episode = |book: &MemoryBook| -> Vec<u32> {
        recall("shader cache", &book.memories, None, one, now)
            .iter()
            .filter(|m| m.kind == MemoryKind::Episodic)
            .map(|m| m.id.0)
            .collect()
    };
    assert_eq!(episode(&book), vec![2], "the recent episode wins");
    let recalled = recall("shader cache", &book.memories, None, one, now);
    assert!(recalled.iter().any(|m| m.id == MemoryId(1)), "facts do not fade");

    for days_ago in [20, 10, 5, 2, 0] {
        book.mark_used(&[MemoryId(0)], now - days_ago * day);
    }
    assert_eq!((book.memories[0].uses, book.memories[0].last_used), (5, now));
    assert_eq!(episode(&book), vec![0], "an old episode in steady use stays");
}
