//! A set that keeps only the latest ids it was given, so what an adapter
//! remembers (messages seen, messages sent) stays bounded however long the
//! account stays linked.

use std::collections::{HashSet, VecDeque};

pub const REMEMBERED: usize = 2048;

#[derive(Debug, Clone)]
pub struct Recent {
    capacity: usize,
    order: VecDeque<String>,
    ids: HashSet<String>,
}

impl Default for Recent {
    fn default() -> Self {
        Self::new(REMEMBERED)
    }
}

impl Recent {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: capacity.max(1), order: VecDeque::new(), ids: HashSet::new() }
    }

    pub fn insert(&mut self, id: String) -> bool {
        if self.ids.contains(&id) {
            return false;
        }
        if self.order.len() == self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.ids.remove(&oldest);
        }
        self.order.push_back(id.clone());
        self.ids.insert(id);
        true
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_new_once() {
        let mut recent = Recent::new(4);
        assert!(recent.insert("a".into()));
        assert!(!recent.insert("a".into()));
        assert!(recent.contains("a"));
    }

    #[test]
    fn past_its_capacity_the_oldest_id_is_forgotten() {
        let mut recent = Recent::new(2);
        for id in ["a", "b", "c"] {
            recent.insert(id.into());
        }
        assert!(!recent.contains("a"));
        assert_eq!(recent.order.len(), recent.ids.len());
        assert!(recent.contains("b") && recent.contains("c"));
    }
}
