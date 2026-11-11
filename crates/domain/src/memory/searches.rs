use std::collections::BTreeSet;

use super::{Memory, MemoryId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAnswer {
    Found(Vec<Memory>),
    AlreadyShown,
    Nothing,
    Exhausted,
}

#[derive(Debug, Clone, Default)]
pub struct MemorySearches {
    shown: BTreeSet<MemoryId>,
    made: usize,
}

impl MemorySearches {
    pub const LIMIT: usize = 3;

    pub fn new(shown: impl IntoIterator<Item = MemoryId>) -> Self {
        Self { shown: shown.into_iter().collect(), made: 0 }
    }

    pub fn answer(&mut self, ranked: Vec<Memory>, limit: usize) -> SearchAnswer {
        if self.made >= Self::LIMIT {
            return SearchAnswer::Exhausted;
        }
        self.made += 1;
        if ranked.is_empty() {
            return SearchAnswer::Nothing;
        }
        let found: Vec<Memory> = ranked.into_iter().filter(|m| !self.shown.contains(&m.id)).take(limit).collect();
        if found.is_empty() {
            return SearchAnswer::AlreadyShown;
        }
        self.shown.extend(found.iter().map(|m| m.id));
        SearchAnswer::Found(found)
    }
}
