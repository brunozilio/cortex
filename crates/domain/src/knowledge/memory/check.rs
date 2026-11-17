use super::search::search;
use super::{Memory, anchors};

pub const MOST_CHECKED: usize = 12;
const MOST_FOUND_BY_WORDS: usize = 4;

pub fn relevant(memories: &[Memory], changed: &[String], diff: &str) -> Vec<Memory> {
    let mut picked: Vec<Memory> = memories
        .iter()
        .filter(|memory| changed.iter().any(|file| anchors::is_about(memory, file)))
        .take(MOST_CHECKED)
        .cloned()
        .collect();
    let words: String = changed.iter().map(String::as_str).chain(changed_lines(diff)).collect::<Vec<_>>().join("\n");
    let scores = search(&words, memories);
    let mut by_words: Vec<usize> = (0..memories.len()).filter(|&at| scores[at] > 0.0).collect();
    by_words.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
    for at in by_words.into_iter().take(MOST_FOUND_BY_WORDS) {
        if picked.len() < MOST_CHECKED && !picked.iter().any(|memory| memory.id == memories[at].id) {
            picked.push(memories[at].clone());
        }
    }
    picked
}

fn changed_lines(diff: &str) -> impl Iterator<Item = &str> {
    diff.lines()
        .filter(|line| {
            (line.starts_with('+') || line.starts_with('-')) && !line.starts_with("+++") && !line.starts_with("---")
        })
        .map(|line| &line[1..])
}

pub fn prompt(base: &str, changed: &[String], memories: &[Memory]) -> Option<String> {
    if memories.is_empty() || changed.is_empty() {
        return None;
    }
    let files: Vec<String> = changed.iter().map(|file| format!("- {file}")).collect();
    let kept: Vec<String> = memories
        .iter()
        .map(|memory| {
            let about = if memory.touches.is_empty() {
                String::new()
            } else {
                format!(" [about {}]", memory.touches.join(", "))
            };
            format!("- {}{about}", anchors::line(memory))
        })
        .collect();
    Some(format!(
        "Check the changes since {base} against what this project's memory says, before they are committed or \
         sent for review. Read them with `git diff {base}` (and `git status` for new files). Do not change any \
         file.\n\nChanged files:\n{files}\n\nMemories about this code (ids in brackets):\n{kept}\n\nFor each \
         memory a change goes against, report one entry:\n✗ <file>:<line>\n  <what the change does>\n  [<id>] \
         <the memory, and whose it is when it says so>\nThen end with one line: `· <n> conflicts · fix them, or \
         say why in the commit message` or `✓ No conflicts with <n> memories`. A change that agrees with a \
         memory, or that a memory does not speak to, is not a conflict. Be brief.",
        files = files.join("\n"),
        kept = kept.join("\n"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Author, MemoryId, MemoryKind};

    fn memory(id: u32, text: &str, touches: &[&str]) -> Memory {
        Memory {
            id: MemoryId(id),
            kind: MemoryKind::Semantic,
            text: text.into(),
            tags: Vec::new(),
            created: 0,
            origin: None,
            author: Author::Agent,
            history: Vec::new(),
            uses: 0,
            last_used: 0,
            touches: touches.iter().map(|t| (*t).to_owned()).collect(),
            credit: None,
        }
    }

    #[test]
    fn the_memories_about_the_changed_files_come_first_then_those_the_diff_mentions() {
        let memories = [
            memory(0, "Keep defaultRetryMax at 4", &["client.go"]),
            memory(1, "The user writes in Portuguese", &[]),
            memory(2, "Backoff waits add up with every retry", &[]),
            memory(3, "Docs are built with mdbook", &["docs"]),
        ];
        let diff = "--- a/client.go\n+++ b/client.go\n-\tdefaultRetryMax = 4\n+\tdefaultRetryMax = 10 // more retry\n";

        let picked = relevant(&memories, &["client.go".into()], diff);

        let ids: Vec<u32> = picked.iter().map(|memory| memory.id.0).collect();
        assert_eq!(ids.first(), Some(&0));
        assert!(ids.contains(&2), "{ids:?}");
        assert!(!ids.contains(&1) && !ids.contains(&3), "{ids:?}");
    }

    #[test]
    fn the_check_names_the_base_the_files_and_each_memory_with_what_it_is_about() {
        let mut credited = memory(1_000_000, "Keep defaultRetryMax at 4", &["client.go"]);
        credited.credit = Some("Maya, 2026-10-09".into());

        let text = prompt("origin/main", &["client.go".into()], &[credited]).expect("prompt");

        assert!(text.contains("git diff origin/main"));
        assert!(text.contains("Changed files:\n- client.go"));
        assert!(text.contains("- [1000000] Keep defaultRetryMax at 4 (Maya, 2026-10-09) [about client.go]"));
        assert!(text.contains("Do not change any file"));
        assert_eq!(prompt("HEAD", &["a.rs".into()], &[]), None);
        assert_eq!(prompt("HEAD", &[], &[memory(0, "x", &[])]), None);
    }
}
