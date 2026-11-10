//! Near duplicates: a new memory whose meaning matches an older one of the
//! same kind becomes a revision of it instead of a second copy.

use super::search::cosine;
use super::{MemoryBook, MemoryId, REVISIONS_KEPT, Revision};

/// Cosine similarity from which two memories say the same thing.
pub const NEAR_DUPLICATE: f32 = 0.92;

impl MemoryBook {
    /// The id the next remembered memory gets.
    pub fn next_id(&self) -> MemoryId {
        MemoryId(self.next.max(self.after_highest()))
    }

    /// Folds each memory from `since` on into the most similar older one of
    /// its kind when their vectors are [`NEAR_DUPLICATE`]: the older keeps
    /// its id, takes the newer text (the old one goes to its history), the
    /// union of tags and the newer vector. Answers (merged, into) pairs.
    /// Memories without a vector are left alone.
    pub fn merge_near_duplicates(&mut self, since: MemoryId, now: i64) -> Vec<(MemoryId, MemoryId)> {
        let fresh: Vec<MemoryId> = self.memories.iter().filter(|m| m.id >= since).map(|m| m.id).collect();
        let mut merged = Vec::new();
        for id in fresh {
            let Some(into) = self.closest_older(id, since) else { continue };
            let Some(at) = self.memories.iter().position(|m| m.id == id) else { continue };
            let newer = self.memories.remove(at);
            let vector = self.vectors.remove(&id);
            let Some(older) = self.memories.iter_mut().find(|m| m.id == into) else { continue };
            let old = std::mem::replace(&mut older.text, newer.text);
            older.history.push(Revision { text: old, until: now });
            let extra = older.history.len().saturating_sub(REVISIONS_KEPT);
            older.history.drain(..extra);
            for tag in newer.tags {
                if !older.tags.contains(&tag) {
                    older.tags.push(tag);
                }
            }
            if let Some(vector) = vector {
                self.vectors.insert(into, vector);
            }
            merged.push((id, into));
        }
        merged
    }

    fn closest_older(&self, id: MemoryId, since: MemoryId) -> Option<MemoryId> {
        let memory = self.memories.iter().find(|m| m.id == id)?;
        let vector = self.vectors.get(&id)?;
        self.memories
            .iter()
            .filter(|m| m.id < since && m.kind == memory.kind)
            .filter_map(|m| Some((m.id, cosine(vector, self.vectors.get(&m.id)?))))
            .filter(|(_, similarity)| *similarity >= NEAR_DUPLICATE)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
}
