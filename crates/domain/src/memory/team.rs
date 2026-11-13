use super::{Author, Memory, MemoryId, MemoryKind, sanitize};

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
    let tags = field("tags")
        .map(|tags| {
            tags.trim()
                .trim_matches(['[', ']'])
                .split(',')
                .map(|tag| tag.trim().to_owned())
                .filter(|tag| !tag.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let text: String = sanitize(body.trim()).into_inner().chars().take(MOST_CHARS).collect();
    let id = MemoryId(FIRST_ID + u32::try_from(at).ok()?);
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
    })
}

pub fn file(memory: &Memory) -> (String, String) {
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
    let text = format!("---\nkind: {kind}\ntags: [{}]\n---\n{}\n", memory.tags.join(", "), memory.text);
    (name, text)
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

        let (name, text) = file(&original);
        let back = parse(0, &name, &text).expect("back");

        assert_eq!(name, "1000000-run-the-linter-before-pushing.md");
        assert_eq!((back.kind, back.tags, back.text), (original.kind, original.tags, original.text));
    }
}
