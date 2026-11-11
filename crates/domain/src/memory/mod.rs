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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<Revision>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub uses: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub last_used: i64,
}

fn is_zero<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

pub const REVISIONS_KEPT: usize = 5;
pub const FORGOTTEN_KEPT_DAYS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub text: String,
    pub until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forgotten {
    pub memory: Memory,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MemoryEdit {
    Remember { kind: MemoryKind, text: String, tags: Vec<String> },
    Forget { id: MemoryId },
    Revise { id: MemoryId, text: String },
    Reclassify { id: MemoryId, kind: MemoryKind },
    Restore { id: MemoryId },
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forgotten: Vec<Forgotten>,
}

impl MemoryBook {
    pub fn apply(&mut self, edits: &[MemoryEdit], by: &Provenance) -> Vec<MemoryId> {
        let expiry = by.now - FORGOTTEN_KEPT_DAYS * 24 * 60 * 60;
        self.forgotten.retain(|f| f.at >= expiry);
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
                        history: Vec::new(),
                        uses: 0,
                        last_used: 0,
                    });
                    touched.push(id);
                }
                MemoryEdit::Forget { id } => {
                    if let Some(at) = self.memories.iter().position(|m| m.id == *id) {
                        let memory = self.memories.remove(at);
                        self.forgotten.push(Forgotten { memory, at: by.now });
                    }
                    self.vectors.remove(id);
                }
                MemoryEdit::Restore { id } => {
                    if let Some(at) = self.forgotten.iter().position(|f| f.memory.id == *id) {
                        let memory = self.forgotten.remove(at).memory;
                        let place = self.memories.partition_point(|m| m.id < memory.id);
                        self.memories.insert(place, memory);
                        touched.push(*id);
                    }
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
                    let old = std::mem::replace(&mut memory.text, text);
                    memory.history.push(Revision { text: old, until: by.now });
                    let extra = memory.history.len().saturating_sub(REVISIONS_KEPT);
                    memory.history.drain(..extra);
                    self.vectors.remove(id);
                    touched.push(*id);
                }
            }
        }
        touched
    }

    pub fn mark_used(&mut self, ids: &[MemoryId], now: i64) {
        for memory in self.memories.iter_mut().filter(|m| ids.contains(&m.id)) {
            memory.uses = memory.uses.saturating_add(1);
            memory.last_used = memory.last_used.max(now);
        }
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
        let forgotten = self.forgotten.iter().map(|f| &f.memory);
        self.memories.iter().chain(forgotten).map(|m| m.id.0 + 1).max().unwrap_or(0)
    }
}

fn clean_text(text: &str) -> Option<String> {
    let text = sanitize(text).into_inner();
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
mod dedup;
mod optimize;
mod recall;
mod sanitize;
pub mod search;

pub use brief::{MEMORY_SEARCH_TOOL, MEMORY_SERVER, MemoryBrief, parse_memory_edits};
pub use optimize::optimize_prompt;
pub use recall::{RecallBudget, links, recall};
pub use sanitize::{REDACTED, Sanitized, sanitize};
pub use search::{Semantic, activate, search};

#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod tests;
