pub mod line_endings;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub diff: String,
    pub untracked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edited {
    pub path: std::path::PathBuf,
    pub line: usize,
    pub removed: usize,
    pub added: usize,
}

impl Edited {
    pub fn replaced(path: std::path::PathBuf, before: &str, old: &str, new: &str) -> Self {
        Self { path, line: before.matches('\n').count() + 1, removed: lines(old), added: lines(new) }
    }

    pub fn written(path: std::path::PathBuf, old: Option<&str>, new: &str) -> Self {
        Self { path, line: 1, removed: old.map_or(0, lines), added: lines(new) }
    }
}

fn lines(text: &str) -> usize {
    text.lines().count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffScope {
    Uncommitted,
    Unstaged,
    Staged,
    Branch,
}

impl DiffScope {
    pub const fn next(self) -> Self {
        match self {
            Self::Uncommitted => Self::Unstaged,
            Self::Unstaged => Self::Staged,
            Self::Staged => Self::Branch,
            Self::Branch => Self::Uncommitted,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Uncommitted => "uncommitted",
            Self::Unstaged => "unstaged",
            Self::Staged => "staged",
            Self::Branch => "branch",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_names_the_line_it_starts_on_and_how_many_lines_it_swapped() {
        let edited = Edited::replaced("a.rs".into(), "one\ntwo\nthr", "ee\nfour", "EE\nFOUR\nFIVE");
        assert_eq!(edited, Edited { path: "a.rs".into(), line: 3, removed: 2, added: 3 });

        let created = Edited::written("b.rs".into(), None, "x\ny\n");
        assert_eq!(created, Edited { path: "b.rs".into(), line: 1, removed: 0, added: 2 });
        assert_eq!(Edited::written("b.rs".into(), Some("old\n"), "").removed, 1);
    }

    #[test]
    fn the_scopes_cycle_and_travel_by_their_label() {
        assert_eq!(DiffScope::Uncommitted.next(), DiffScope::Unstaged);
        assert_eq!(DiffScope::Branch.next(), DiffScope::Uncommitted);
        assert_eq!(serde_json::to_value(DiffScope::Staged).ok(), Some(serde_json::json!("staged")));
    }
}
