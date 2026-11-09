//! The graph's links and what a prompt recalls.

use std::collections::{BTreeMap, BTreeSet};

use super::search::{Semantic, search};
use super::{Memory, MemoryKind};

pub fn links(memories: &[Memory]) -> Vec<(usize, usize)> {
    let mut by_tag: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, memory) in memories.iter().enumerate() {
        for tag in &memory.tags {
            by_tag.entry(tag.as_str()).or_default().push(i);
        }
    }
    let mut edges = BTreeSet::new();
    for members in by_tag.values_mut() {
        members.sort_by_key(|&i| memories[i].id);
        for pair in members.windows(2) {
            edges.insert((pair[0].min(pair[1]), pair[0].max(pair[1])));
        }
    }
    edges.into_iter().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallBudget {
    pub facts: usize,
    pub episodes: usize,
}

impl Default for RecallBudget {
    fn default() -> Self {
        Self { facts: 40, episodes: 8 }
    }
}

impl RecallBudget {
    pub const WITH_TOOL: Self = Self { facts: 15, episodes: 3 };
}

pub fn recall(prompt: &str, memories: &[Memory], semantic: Option<Semantic>, budget: RecallBudget) -> Vec<Memory> {
    let scores = search(prompt, memories, semantic);
    let ranked = |episodic: bool, take: usize| -> Vec<usize> {
        let mut picked: Vec<usize> =
            (0..memories.len()).filter(|&i| (memories[i].kind == MemoryKind::Episodic) == episodic).collect();
        picked.sort_by(|&a, &b| {
            scores[b].total_cmp(&scores[a]).then(memories[b].created.cmp(&memories[a].created)).then(b.cmp(&a))
        });
        picked.truncate(take);
        picked
    };
    let mut picked = ranked(false, budget.facts);
    picked.extend(ranked(true, budget.episodes));
    picked.sort_by_key(|&i| (memories[i].kind, memories[i].id));
    picked.into_iter().map(|i| memories[i].clone()).collect()
}
