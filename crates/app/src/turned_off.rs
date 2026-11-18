use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use domain::features::Features;
use platform::sync::Locked;

#[derive(Debug, Clone, Default)]
pub struct TurnedOff {
    always: BTreeSet<String>,
    in_settings: Arc<Mutex<BTreeSet<String>>>,
}

impl TurnedOff {
    pub fn insert(&mut self, name: String) -> bool {
        self.always.insert(name)
    }

    pub fn extend(&mut self, names: impl IntoIterator<Item = String>) {
        self.always.extend(names);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.always.contains(name) || self.in_settings.locked().contains(name)
    }

    pub fn follow(&self, features: &Features) {
        let declared = features.off().filter(|name| domain::features::known(name).is_some());
        *self.in_settings.locked() = declared.map(str::to_owned).collect();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn features(flags: serde_json::Value) -> Features {
        Features::default().layered(&flags, true).expect("flags")
    }

    #[test]
    fn what_the_settings_turn_off_follows_them_and_what_the_program_turned_off_stays() {
        let mut off = TurnedOff::default();
        off.insert("memory_search".into());
        let shared = off.clone();

        off.follow(&features(json!({ "suggested-rules": false, "budget": true })));
        assert!(shared.contains("suggested-rules"), "every clone sees it");
        assert!(!shared.contains("budget"));

        off.follow(&features(json!({ "suggested-rules": true })));
        assert!(!shared.contains("suggested-rules"), "turned back on");
        assert!(shared.contains("memory_search"), "the settings never said anything about it");
    }

    #[test]
    fn only_a_declared_flag_turns_anything_off_from_the_settings() {
        let off = TurnedOff::default();
        off.follow(&features(
            json!({ "sessions": false, "suggested_rules": false, "typo": false, "memory": false, "trace": false }),
        ));
        assert!(off.contains("trace"));
        for unknown in ["sessions", "suggested_rules", "suggested-rules", "typo", "memory"] {
            assert!(!off.contains(unknown), "{unknown} is not a flag /features knows");
        }
    }
}
