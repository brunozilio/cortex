//! Memory a member shared beyond themselves (`sharing::MemoryScope`): with
//! everyone in the project, or the whole organization. The organization's
//! server keeps it; every run of the project recalls it.

use serde::{Deserialize, Serialize};

use super::{Memory, MemoryId};
use crate::ids::UserId;
use crate::sharing::MemoryScope;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedMemory {
    pub scope: MemoryScope,
    pub memory: Memory,
    pub author: UserId,
    pub author_name: String,
}

pub fn next_id(shared: &[SharedMemory]) -> MemoryId {
    MemoryId(shared.iter().map(|s| s.memory.id.0).max().map_or(1, |high| high + 1))
}

pub fn listing(shared: &[SharedMemory]) -> Option<String> {
    if shared.is_empty() {
        return None;
    }
    let mut out = String::from("Shared by teammates (the project's and the organization's memory):");
    for item in shared {
        let whose = match item.scope {
            MemoryScope::Organization => "organization",
            _ => "project",
        };
        out.push_str(&format!("\n- ({whose}, from {}) {}", item.author_name, item.memory.text));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Author, MemoryKind};

    fn shared(id: u32, scope: MemoryScope, text: &str) -> SharedMemory {
        let memory = Memory {
            id: MemoryId(id),
            kind: MemoryKind::Procedural,
            text: text.into(),
            tags: vec![],
            created: 1,
            origin: None,
            author: Author::User,
        };
        SharedMemory { scope, memory, author: UserId::new(), author_name: "Ana".into() }
    }

    #[test]
    fn ids_follow_the_highest() {
        assert_eq!(next_id(&[]), MemoryId(1));
        let both = [shared(4, MemoryScope::Project, "a"), shared(2, MemoryScope::Organization, "b")];
        assert_eq!(next_id(&both), MemoryId(5));
    }

    #[test]
    fn the_listing_names_scope_and_author() {
        assert_eq!(listing(&[]), None);
        let text = listing(&[shared(1, MemoryScope::Organization, "Deploys go through the release train.")]);
        assert!(text.expect("listed").contains("(organization, from Ana) Deploys go through the release train."));
    }
}
