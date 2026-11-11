use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub diff: String,
    pub untracked: Vec<String>,
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
    fn the_scopes_cycle_and_travel_by_their_label() {
        assert_eq!(DiffScope::Uncommitted.next(), DiffScope::Unstaged);
        assert_eq!(DiffScope::Branch.next(), DiffScope::Uncommitted);
        assert_eq!(serde_json::to_value(DiffScope::Staged).ok(), Some(serde_json::json!("staged")));
    }
}
