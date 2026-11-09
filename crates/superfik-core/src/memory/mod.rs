//! Project memory: what agents remember between sessions.
//!
//! One scope only: every session in a project shares the same memory,
//! private to its owner. Memories are episodic (what happened), semantic
//! (facts about the project or the user) or procedural (how to do things
//! here). The agent writes edits through a JSON file at the end of a run
//! (see [`MemoryBrief::instructions`]); the user adds and forgets them by
//! hand. [`activate`] scores memories against a text: the search field
//! and prompt recall share it, so what lights up is what the agent gets.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const MAX_MEMORY_CHARS: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Episodic,
    Semantic,
    Procedural,
}

impl MemoryKind {
    pub const ALL: [MemoryKind; 3] = [MemoryKind::Episodic, MemoryKind::Semantic, MemoryKind::Procedural];

    pub fn key(self) -> &'static str {
        match self {
            MemoryKind::Episodic => "episodic",
            MemoryKind::Semantic => "semantic",
            MemoryKind::Procedural => "procedural",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key().eq_ignore_ascii_case(key.trim()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    Agent,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    pub id: MemoryId,
    pub kind: MemoryKind,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created: i64,
    #[serde(default)]
    pub origin: Option<String>,
    pub author: Author,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MemoryEdit {
    Remember { kind: MemoryKind, text: String, tags: Vec<String> },
    Forget { id: MemoryId },
    Revise { id: MemoryId, text: String },
    Reclassify { id: MemoryId, kind: MemoryKind },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub author: Author,
    pub now: i64,
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryBook {
    #[serde(default)]
    pub next: u32,
    #[serde(default)]
    pub memories: Vec<Memory>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vectors: BTreeMap<MemoryId, Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl MemoryBook {
    pub fn apply(&mut self, edits: &[MemoryEdit], by: &Provenance) -> Vec<MemoryId> {
        let mut touched = Vec::new();
        for edit in edits {
            match edit {
                MemoryEdit::Remember { kind, text, tags } => {
                    let Some(text) = clean_text(text) else { continue };
                    if self.memories.iter().any(|m| m.kind == *kind && same_text(&m.text, &text)) {
                        continue;
                    }
                    let id = MemoryId(self.next.max(self.after_highest()));
                    self.next = id.0 + 1;
                    self.memories.push(Memory {
                        id,
                        kind: *kind,
                        text,
                        tags: clean_tags(tags),
                        created: by.now,
                        origin: by.origin.clone(),
                        author: by.author,
                    });
                    touched.push(id);
                }
                MemoryEdit::Forget { id } => {
                    self.memories.retain(|m| m.id != *id);
                    self.vectors.remove(id);
                }
                MemoryEdit::Reclassify { id, kind } => {
                    if let Some(memory) = self.memories.iter_mut().find(|m| m.id == *id && m.kind != *kind) {
                        memory.kind = *kind;
                        touched.push(*id);
                    }
                }
                MemoryEdit::Revise { id, text } => {
                    let (Some(text), Some(memory)) = (clean_text(text), self.memories.iter_mut().find(|m| m.id == *id))
                    else {
                        continue;
                    };
                    memory.text = text;
                    self.vectors.remove(id);
                    touched.push(*id);
                }
            }
        }
        touched
    }

    pub fn unembedded(&mut self, model: &str) -> Vec<(MemoryId, String)> {
        if self.model.as_deref() != Some(model) {
            self.vectors.clear();
            self.model = Some(model.to_owned());
        }
        self.memories.iter().filter(|m| !self.vectors.contains_key(&m.id)).map(|m| (m.id, m.text.clone())).collect()
    }

    pub fn aligned_vectors(&self) -> Vec<Option<&[f32]>> {
        self.memories.iter().map(|m| self.vectors.get(&m.id).map(Vec::as_slice)).collect()
    }

    fn after_highest(&self) -> u32 {
        self.memories.iter().map(|m| m.id.0 + 1).max().unwrap_or(0)
    }
}

fn clean_text(text: &str) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= MAX_MEMORY_CHARS {
        return Some(text);
    }
    let cut: String = text.chars().take(MAX_MEMORY_CHARS).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    Some(format!("{cut}…"))
}

fn same_text(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    tags.iter()
        .map(|tag| tag.trim().trim_start_matches('#').to_lowercase())
        .filter(|tag| !tag.is_empty() && seen.insert(tag.clone()))
        .take(8)
        .collect()
}

mod brief;
mod optimize;
mod recall;
pub mod search;
pub mod shared;

pub use brief::{MEMORY_SEARCH_TOOL, MEMORY_SERVER, MemoryBrief, MemoryTool, parse_memory_edits};
pub use optimize::optimize_prompt;
pub use recall::{RecallBudget, links, recall};
pub use search::{Semantic, activate, search};

#[cfg(test)]
mod tests;
