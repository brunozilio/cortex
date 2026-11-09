//! What a client knows about each engine (can it run here?) and the
//! well-known reasons a run cannot start.

use serde::{Deserialize, Serialize};

use crate::model::{EngineKind, ModelOption};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInfo {
    pub kind: EngineKind,
    pub installed: bool,
    pub implemented: bool,
    pub models: Vec<ModelOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Ready,
    NotInstalled,
    Soon { installed: bool },
}

impl EngineInfo {
    pub fn availability(&self) -> Availability {
        match (self.implemented, self.installed) {
            (true, true) => Availability::Ready,
            (true, false) => Availability::NotInstalled,
            (false, installed) => Availability::Soon { installed },
        }
    }

    pub fn is_selectable(&self) -> bool {
        self.implemented
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineProblem {
    NotInstalled,
    NotLoggedIn,
    SessionNotFound,
}

impl EngineProblem {
    pub fn message(self, engine: EngineKind) -> String {
        let name = engine.label();
        match (self, engine) {
            (Self::NotInstalled, EngineKind::Native) => {
                "The built-in AI engine is unavailable. Restart Superfik and try again.".to_owned()
            }
            (Self::NotInstalled, _) => format!("{name} isn't installed on this machine."),
            (Self::NotLoggedIn, EngineKind::Native) => {
                "Connect an AI account in Settings → AI and try again.".to_owned()
            }
            (Self::NotLoggedIn, _) => format!("{name} isn't logged in."),
            (Self::SessionNotFound, _) => {
                format!("The previous {name} conversation was not found; your next message starts a new one.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(installed: bool, implemented: bool) -> EngineInfo {
        EngineInfo { kind: EngineKind::Codex, installed, implemented, models: Vec::new() }
    }

    #[test]
    fn availability_distinguishes_missing_from_unimplemented() {
        assert_eq!(info(true, true).availability(), Availability::Ready);
        assert_eq!(info(false, true).availability(), Availability::NotInstalled);
        assert_eq!(info(true, false).availability(), Availability::Soon { installed: true });
        assert_eq!(info(false, false).availability(), Availability::Soon { installed: false });
        assert!(info(false, true).is_selectable());
        assert!(!info(true, false).is_selectable());
    }

    #[test]
    fn problems_explain_what_to_do() {
        assert!(EngineProblem::NotInstalled.message(EngineKind::Native).contains("Restart Superfik"));
        assert!(EngineProblem::NotLoggedIn.message(EngineKind::Native).contains("Settings → AI"));
        assert!(EngineProblem::SessionNotFound.message(EngineKind::Native).contains("new one"));
        assert_eq!(EngineProblem::NotInstalled.message(EngineKind::Kiro), "Kiro isn't installed on this machine.");
    }
}
