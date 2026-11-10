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

const DAY: f32 = 24.0 * 60.0 * 60.0;
/// Days for an unused episode's weight to halve.
const EPISODE_HALF_LIFE_DAYS: f32 = 30.0;
/// How much repeated use counts against fading.
const USE_WEIGHT: f32 = 0.5;

/// How much an episode still weighs at `now`: it halves every
/// [`EPISODE_HALF_LIFE_DAYS`], and use adds back `USE_WEIGHT·ln(1+uses)`,
/// itself fading from the last use. Facts and procedures keep 1.0.
pub fn retention(memory: &Memory, now: i64) -> f32 {
    if memory.kind != MemoryKind::Episodic {
        return 1.0;
    }
    let decay =
        |since: i64| (-(std::f32::consts::LN_2) * (now - since).max(0) as f32 / DAY / EPISODE_HALF_LIFE_DAYS).exp();
    let used = if memory.uses == 0 { 0.0 } else { USE_WEIGHT * (memory.uses as f32).ln_1p() * decay(memory.last_used) };
    decay(memory.created) + used
}

pub fn recall(
    prompt: &str,
    memories: &[Memory],
    semantic: Option<Semantic>,
    budget: RecallBudget,
    now: i64,
) -> Vec<Memory> {
    let relevance = search(prompt, memories, semantic);
    let kept: Vec<f32> = memories.iter().map(|m| retention(m, now)).collect();
    let scores: Vec<f32> = relevance.iter().zip(&kept).map(|(score, kept)| score * kept).collect();
    let ranked = |episodic: bool, take: usize| -> Vec<usize> {
        let mut picked: Vec<usize> =
            (0..memories.len()).filter(|&i| (memories[i].kind == MemoryKind::Episodic) == episodic).collect();
        picked.sort_by(|&a, &b| {
            scores[b]
                .total_cmp(&scores[a])
                .then(kept[b].total_cmp(&kept[a]))
                .then(memories[b].created.cmp(&memories[a].created))
                .then(b.cmp(&a))
        });
        picked.truncate(take);
        picked
    };
    let mut picked = ranked(false, budget.facts);
    picked.extend(ranked(true, budget.episodes));
    picked.sort_by_key(|&i| (memories[i].kind, memories[i].id));
    picked.into_iter().map(|i| memories[i].clone()).collect()
}
