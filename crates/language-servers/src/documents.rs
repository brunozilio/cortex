use std::collections::{HashMap, HashSet};

use serde_json::Value;

pub(crate) const MOST_OPEN: usize = 64;
pub(crate) const MOST_PUBLISHED: usize = 256;

struct Open {
    version: i64,
    used: u64,
}

#[derive(Default)]
pub(crate) struct Documents {
    open: HashMap<String, Open>,
    uses: u64,
}

impl Documents {
    pub(crate) fn version(&self, uri: &str) -> Option<i64> {
        self.open.get(uri).map(|open| open.version)
    }

    pub(crate) fn keep(&mut self, uri: String, version: i64) {
        self.uses += 1;
        self.open.insert(uri, Open { version, used: self.uses });
    }

    pub(crate) fn make_room(&mut self) -> Option<String> {
        if self.open.len() < MOST_OPEN {
            return None;
        }
        let stalest = self.open.iter().min_by_key(|(_, open)| open.used).map(|(uri, _)| uri.clone())?;
        self.open.remove(&stalest);
        Some(stalest)
    }

    pub(crate) fn uris(&self) -> HashSet<String> {
        self.open.keys().cloned().collect()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.open.len()
    }
}

pub(crate) struct Diagnosed {
    pub(crate) version: Option<i64>,
    pub(crate) items: Vec<Value>,
    told: u64,
}

#[derive(Default)]
pub(crate) struct Published {
    of: HashMap<String, Diagnosed>,
    told: u64,
}

impl Published {
    pub(crate) fn get(&self, uri: &str) -> Option<&Diagnosed> {
        self.of.get(uri)
    }

    pub(crate) fn all(&self) -> Vec<(String, Vec<Value>)> {
        let mut all: Vec<(String, Vec<Value>)> =
            self.of.iter().map(|(uri, found)| (uri.clone(), found.items.clone())).collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }

    pub(crate) fn forget(&mut self, uri: &str) {
        self.of.remove(uri);
    }

    pub(crate) fn keep(&mut self, uri: String, version: Option<i64>, items: Vec<Value>) {
        self.told += 1;
        self.of.insert(uri, Diagnosed { version, items, told: self.told });
    }

    pub(crate) fn is_over(&self) -> bool {
        self.of.len() > MOST_PUBLISHED
    }

    pub(crate) fn trim(&mut self, open: &HashSet<String>) {
        while self.is_over() {
            let closed = self.of.iter().filter(|(uri, _)| !open.contains(*uri));
            let Some(oldest) = closed.min_by_key(|(_, found)| found.told).map(|(uri, _)| uri.clone()) else { return };
            self.of.remove(&oldest);
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.of.len()
    }
}
