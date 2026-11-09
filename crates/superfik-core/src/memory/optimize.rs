//! Tidying a project's memory: one agent run reads every memory and hands
//! back edits that merge, correct, reclassify and drop, through the same
//! edits file as any run (see [`MemoryBrief::instructions`]).
//!
//! [`MemoryBrief::instructions`]: super::MemoryBrief::instructions

use super::Memory;

pub fn optimize_prompt(memories: &[Memory]) -> String {
    let mut out = String::from(
        "You maintain this project's long-term memory. It is listed below, one memory per line as \
         [id] kind (tags): text. Tidy it without losing anything that still matters:\n\
         - Merge memories that say the same thing: revise one to hold the merged text and forget the others.\n\
         - Forget memories that are trivial, obsolete or contradicted by newer ones.\n\
         - Reclassify memories filed under the wrong kind: episodic is what happened, semantic is a durable \
         fact about the project or the user, procedural is how to do something here.\n\
         - Fold many small episodic memories about one topic into a single semantic or episodic summary.\n\
         - Make wordy memories short and precise; keep names, paths, commands and numbers exact.\n\
         Never invent facts, and do not look at or change the project's files. If the memory is already \
         tidy, change nothing. Write every change to the memory file described at the end; do not add a \
         memory about this tidying itself.\n\nMemory:",
    );
    for m in memories {
        let tags = if m.tags.is_empty() { String::new() } else { format!(" ({})", m.tags.join(", ")) };
        out.push_str(&format!("\n[{}] {}{tags}: {}", m.id.0, m.kind.key(), m.text));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, MemoryId, MemoryKind};

    #[test]
    fn every_memory_is_listed_with_its_id_kind_and_tags() {
        let memory = |id, kind, tags: &[&str]| Memory {
            id: MemoryId(id),
            kind,
            text: format!("m{id}"),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            created: 0,
            origin: None,
            author: Author::Agent,
        };
        let prompt =
            optimize_prompt(&[memory(3, MemoryKind::Semantic, &["ci", "lint"]), memory(7, MemoryKind::Episodic, &[])]);
        assert!(prompt.contains("\n[3] semantic (ci, lint): m3\n[7] episodic: m7"));
        assert!(prompt.contains("Merge memories"));
    }
}
