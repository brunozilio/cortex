//! Comments on a shared session: what members who can see it say about
//! it, each placed after the turn it follows. The organization's server
//! keeps them.

use serde::{Deserialize, Serialize};

use crate::ids::UserId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub author: UserId,
    pub author_name: String,
    pub at_unix: u64,
    pub after_turn: usize,
    pub text: String,
}

pub const MAX_COMMENT_CHARS: usize = 2_000;

pub fn tidy(text: &str) -> Option<String> {
    let text: String = text.trim().chars().take(MAX_COMMENT_CHARS).collect();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_are_trimmed_cut_and_never_empty() {
        assert_eq!(tidy("  Looks good  ").as_deref(), Some("Looks good"));
        assert_eq!(tidy("   "), None);
        assert_eq!(tidy(&"a".repeat(3_000)).map(|t| t.len()), Some(MAX_COMMENT_CHARS));
    }
}
