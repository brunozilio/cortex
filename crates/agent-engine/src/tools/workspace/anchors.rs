use std::path::Path;

use platform::sync::Locked;

use super::Workspace;

impl Workspace {
    pub fn anchored(&self, path: &Path) -> String {
        if self.anchored_memories.is_empty() {
            return String::new();
        }
        let Some(file) = domain::memory::anchors::relative_to(&self.cwd, path) else { return String::new() };
        let mut shown = self.tracking.seen_memories.locked();
        domain::memory::anchors::about_file(&self.anchored_memories, &file, &mut shown)
            .map(|block| format!("\n\n{block}"))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use domain::memory::{Author, Memory, MemoryId, MemoryKind};

    use super::*;

    #[test]
    fn a_memory_about_a_file_is_shown_the_first_time_the_file_is_touched() {
        let dir = tempfile::tempdir().expect("dir");
        let mut workspace = Workspace::new(dir.path());
        workspace.anchored_memories = vec![Memory {
            id: MemoryId(4),
            kind: MemoryKind::Semantic,
            text: "Keep retries at 4".into(),
            tags: Vec::new(),
            created: 0,
            origin: None,
            author: Author::Agent,
            history: Vec::new(),
            uses: 0,
            last_used: 0,
            touches: vec!["src".into()],
            credit: None,
        }];

        let first = workspace.anchored(&dir.path().join("src/client.rs"));

        assert!(first.starts_with("\n\n<memory>Kept from earlier sessions about src/client.rs"), "{first}");
        assert!(first.contains("[4] Keep retries at 4"));
        assert_eq!(workspace.anchored(&dir.path().join("src/client.rs")), "");
        assert_eq!(workspace.anchored(&dir.path().join("README.md")), "");
        assert_eq!(Workspace::new(dir.path()).anchored(&dir.path().join("src/a.rs")), "");
    }
}
