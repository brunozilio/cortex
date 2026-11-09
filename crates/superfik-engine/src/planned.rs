//! Engines Superfik knows about but has no adapter for yet. They are
//! listed (as "Soon") so the picker shows what is coming and whether the
//! CLI is already installed; they can never start a run.

use superfik_core::{EngineInfo, EngineKind, ModelOption};

use crate::locate::{self, BinarySpec};

pub const CODEX: BinarySpec =
    BinarySpec { names: &["codex"], env_override: "SUPERFIK_CODEX_BIN", home_dirs: &[".codex/bin"] };
pub const KIRO: BinarySpec =
    BinarySpec { names: &["kiro-cli", "kiro"], env_override: "SUPERFIK_KIRO_BIN", home_dirs: &[".kiro/bin"] };
pub const COPILOT: BinarySpec =
    BinarySpec { names: &["copilot"], env_override: "SUPERFIK_COPILOT_BIN", home_dirs: &[] };

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedEngine {
    pub kind: EngineKind,
    pub installed: bool,
}

impl PlannedEngine {
    pub fn detect_all() -> Vec<Self> {
        [(EngineKind::Codex, CODEX), (EngineKind::Kiro, KIRO), (EngineKind::Copilot, COPILOT)]
            .into_iter()
            .map(|(kind, spec)| Self { kind, installed: locate::locate(&spec).is_some() })
            .collect()
    }

    pub fn info(&self) -> EngineInfo {
        EngineInfo {
            kind: self.kind,
            installed: self.installed,
            implemented: false,
            models: vec![ModelOption::new(self.kind, "default", self.kind.label())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_engines_are_never_selectable() {
        let info = PlannedEngine { kind: EngineKind::Kiro, installed: true }.info();
        assert!(!info.is_selectable());
        assert!(info.installed);
        assert_eq!(info.models[0].engine, EngineKind::Kiro);
        assert_eq!(PlannedEngine::detect_all().len(), 3);
    }

    #[test]
    fn kiro_prefers_kiro_cli() {
        assert_eq!(KIRO.default_name(), "kiro-cli");
    }
}
