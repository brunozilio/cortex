use super::decision_note::DecisionNote;
use super::{Author, Memory, MemoryId, MemoryKind, anchors, sanitize};

pub const FIRST_ID: u32 = 1_000_000;
const MOST_CHARS: usize = 2_000;

pub fn is_team(id: MemoryId) -> bool {
    id.0 >= FIRST_ID
}

pub fn parse(at: usize, name: &str, text: &str) -> Option<Memory> {
    let (front, body) = match text.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---")) {
        Some((front, body)) => (front, body.trim_start_matches(['-', '\n'])),
        None => ("", text),
    };
    let field = |key: &str| {
        front.lines().find_map(|line| line.strip_prefix(key).and_then(|rest| rest.trim_start().strip_prefix(':')))
    };
    let kind = match field("kind").map(str::trim) {
        Some("procedural") => MemoryKind::Procedural,
        Some("episodic") => MemoryKind::Episodic,
        _ => MemoryKind::Semantic,
    };
    let list = |key: &str| -> Vec<String> {
        field(key)
            .map(|items| {
                items
                    .trim()
                    .trim_matches(['[', ']'])
                    .split(',')
                    .map(|item| item.trim().to_owned())
                    .filter(|item| !item.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let body = DecisionNote::from_markdown(body).map_or_else(|| body.trim().to_owned(), |decision| decision.text());
    let text: String = sanitize(&body).into_inner().chars().take(MOST_CHARS).collect();
    let id = MemoryId(FIRST_ID + u32::try_from(at).ok()?);
    let said = |key: &str| field(key).map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());
    let credit = match (said("author"), said("date")) {
        (Some(author), Some(date)) => Some(format!("{author}, {date}")),
        (author, date) => author.or(date),
    };
    let tags = list("tags");
    let touches = anchors::clean(&list("touches"));
    (!text.is_empty()).then(|| Memory {
        id,
        kind,
        text,
        tags,
        created: 0,
        origin: Some(format!("team:{name}")),
        author: Author::User,
        history: Vec::new(),
        uses: 0,
        last_used: 0,
        touches,
        credit,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sharer {
    pub author: Option<String>,
    pub date: Option<String>,
    pub structured: bool,
}

pub fn file(memory: &Memory, by: &Sharer) -> (String, String) {
    let words: Vec<String> = memory
        .text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(6)
        .map(str::to_ascii_lowercase)
        .collect();
    let name = format!("{}-{}.md", memory.id.0, words.join("-"));
    let kind = match memory.kind {
        MemoryKind::Semantic => "semantic",
        MemoryKind::Procedural => "procedural",
        MemoryKind::Episodic => "episodic",
    };
    let mut front = format!("kind: {kind}\ntags: [{}]\n", memory.tags.join(", "));
    if !memory.touches.is_empty() {
        front.push_str(&format!("touches: [{}]\n", memory.touches.join(", ")));
    }
    for (key, value) in [("author", &by.author), ("date", &by.date)] {
        if let Some(value) = value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            front.push_str(&format!("{key}: {value}\n"));
        }
    }
    let body = match DecisionNote::parse(&memory.text).filter(|_| by.structured) {
        Some(decision) => decision.markdown(),
        None => format!("{}\n", memory.text),
    };
    (name, format!("---\n{front}---\n{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checked_in_file_becomes_a_read_only_memory() {
        let text = "---\nkind: procedural\ntags: [release, ci]\n---\nRelease with `make release`; never tag by hand.\n";

        let memory = parse(3, "release.md", text).expect("memory");

        assert_eq!(memory.id, MemoryId(FIRST_ID + 3));
        assert!(is_team(memory.id) && !is_team(MemoryId(7)));
        assert_eq!(memory.kind, MemoryKind::Procedural);
        assert_eq!(memory.tags, ["release", "ci"]);
        assert_eq!(memory.text, "Release with `make release`; never tag by hand.");
        assert_eq!(memory.origin.as_deref(), Some("team:release.md"));
    }

    #[test]
    fn a_plain_file_is_a_fact_and_secrets_in_it_are_hidden() {
        let memory = parse(0, "db.md", "The staging password=hunter22 lives in the vault.").expect("memory");
        assert_eq!(memory.kind, MemoryKind::Semantic);
        assert!(!memory.text.contains("hunter22"));
        assert!(parse(1, "empty.md", "---\nkind: semantic\n---\n").is_none());
    }

    #[test]
    fn a_memory_is_written_as_a_file_that_reads_back_the_same() {
        let original =
            parse(0, "x.md", "---\nkind: procedural\ntags: [ci]\n---\nRun the linter before pushing.").expect("memory");

        let (name, text) = file(&original, &Sharer::default());
        let back = parse(0, &name, &text).expect("back");

        assert_eq!(name, "1000000-run-the-linter-before-pushing.md");
        assert_eq!((back.kind, back.tags, back.text), (original.kind, original.tags, original.text));
        assert_eq!(back.credit, None);
    }

    #[test]
    fn a_shared_decision_keeps_its_files_author_and_sections() {
        let mut decision = parse(0, "x.md", "Decided: Keep retries at 4. Ruled out: 10, one call could block an hour.")
            .expect("memory");
        decision.touches = vec!["client.go".into()];
        let by = Sharer { author: Some("Maya".into()), date: Some("2026-10-09".into()), structured: true };

        let (_, text) = file(&decision, &by);
        let back = parse(0, "x.md", &text).expect("back");

        assert!(text.contains("touches: [client.go]\nauthor: Maya\ndate: 2026-10-09\n---\n## Decided\n"), "{text}");
        assert!(text.contains("## Tried and ruled out\n- 10, one call could block an hour\n"), "{text}");
        assert_eq!(back.text, "Decided: Keep retries at 4. Ruled out: 10, one call could block an hour.");
        assert_eq!(back.touches, ["client.go"]);
        assert_eq!(back.credit.as_deref(), Some("Maya, 2026-10-09"));

        let (_, plain) = file(&decision, &Sharer::default());
        assert!(plain.ends_with("---\nDecided: Keep retries at 4. Ruled out: 10, one call could block an hour.\n"));
    }
}
