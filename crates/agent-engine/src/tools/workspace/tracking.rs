use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use crate::prompt::rules::Rule;
use crate::tools::read_files::{Freshness, ReadFiles};
use crate::tools::seen_files::SeenFiles;

#[derive(Debug, Default)]
pub(crate) struct Tracking {
    pub(crate) read: Arc<ReadFiles>,
    pub(crate) refusals: Mutex<domain::interception::Refusals>,
    pub(crate) seen_files: Arc<SeenFiles>,
    pub(crate) seen_rules: Arc<Mutex<HashSet<PathBuf>>>,
    pub(crate) seen_memories: Arc<Mutex<HashSet<domain::MemoryId>>>,
    pub(crate) shells: Mutex<Vec<String>>,
    heard: Mutex<domain::egress::Heard>,
}

impl Tracking {
    pub(crate) fn fork(&self) -> Self {
        Self { read: Arc::clone(&self.read), ..Self::default() }
    }

    pub(crate) fn mark_read(&self, path: &Path) {
        self.read.mark(path, true);
    }

    pub(crate) fn mark_partly_read(&self, path: &Path) {
        self.read.mark(path, false);
    }

    pub(crate) fn freshness(&self, path: &Path) -> Freshness {
        self.read.freshness(path)
    }

    pub(crate) fn has_read(&self, path: &Path) -> bool {
        self.freshness(path) != Freshness::Unread
    }

    pub(crate) fn forget_seen_files(&self) {
        self.seen_files.forget();
    }

    pub(crate) fn count_refusal(&self, refused: bool) {
        let mut refusals = self.refusals.locked();
        if refused { refusals.refused() } else { refusals.went_ahead() }
    }

    pub(crate) fn refusals_stop(&self) -> Option<String> {
        self.refusals.locked().stop()
    }

    pub(crate) fn hear(&self, heard: domain::egress::Heard) {
        *self.heard.locked() = heard;
    }

    pub(crate) fn heard(&self) -> domain::egress::Heard {
        self.heard.locked().clone()
    }

    pub(crate) fn started_shell(&self, id: &str) {
        self.shells.locked().push(id.to_owned());
    }

    pub(crate) fn started_shells(&self) -> Vec<String> {
        self.shells.locked().clone()
    }

    pub(crate) fn rule_instructions(&self, rules: &[Rule]) -> String {
        let seen = self.seen_rules.locked();
        rules
            .iter()
            .filter(|r| r.error.is_none() && (r.patterns.is_empty() || seen.contains(&r.path)))
            .map(|r| format!("\n\n# Project rule: {}\n{}", r.path.display(), r.text))
            .collect::<Vec<_>>()
            .join("")
    }
}
