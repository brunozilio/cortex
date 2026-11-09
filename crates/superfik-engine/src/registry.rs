//! Engines available to a backend, looked up by kind.

use std::sync::Arc;

use superfik_core::{EngineInfo, EngineKind, ModelOption};

use crate::engine::{AgentEngine, EngineError};
use crate::planned::PlannedEngine;

#[derive(Default, Clone)]
pub struct EngineRegistry {
    engines: Vec<Arc<dyn AgentEngine>>,
    planned: Vec<PlannedEngine>,
}

impl std::fmt::Debug for EngineRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.kinds()).finish()
    }
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, engine: Arc<dyn AgentEngine>) {
        let kind = engine.kind();
        match self.engines.iter_mut().find(|existing| existing.kind() == kind) {
            Some(slot) => *slot = engine,
            None => self.engines.push(engine),
        }
    }

    pub fn with(mut self, engine: Arc<dyn AgentEngine>) -> Self {
        self.register(engine);
        self
    }

    pub fn get(&self, kind: EngineKind) -> Option<Arc<dyn AgentEngine>> {
        self.engines.iter().find(|engine| engine.kind() == kind).cloned()
    }

    pub fn require(&self, kind: EngineKind) -> Result<Arc<dyn AgentEngine>, EngineError> {
        self.get(kind).ok_or(EngineError::UnknownEngine(kind.label()))
    }

    pub fn kinds(&self) -> Vec<EngineKind> {
        self.engines.iter().map(|engine| engine.kind()).collect()
    }

    pub fn models(&self) -> Vec<ModelOption> {
        self.engines.iter().flat_map(|engine| engine.models()).collect()
    }

    pub fn with_planned(mut self, planned: Vec<PlannedEngine>) -> Self {
        self.planned = planned;
        self
    }

    pub fn infos(&self) -> Vec<EngineInfo> {
        let real = self.engines.iter().map(|engine| EngineInfo {
            kind: engine.kind(),
            installed: engine.is_installed(),
            implemented: true,
            models: engine.models(),
        });
        let kinds = self.kinds();
        let planned = self.planned.iter().filter(|p| !kinds.contains(&p.kind)).map(PlannedEngine::info);
        real.chain(planned).collect()
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::EngineEvent;

    use super::*;
    use crate::fake::FakeEngine;

    fn fake(kind: EngineKind, model: &str) -> Arc<dyn AgentEngine> {
        Arc::new(FakeEngine::new(kind, vec![ModelOption::new(kind, model, model)], vec![EngineEvent::Cancelled]))
    }

    #[test]
    fn looks_up_by_kind() {
        let registry = EngineRegistry::new().with(fake(EngineKind::Native, "opus"));
        assert!(registry.get(EngineKind::Native).is_some());
        assert!(registry.get(EngineKind::Codex).is_none());
        assert!(matches!(registry.require(EngineKind::Codex), Err(EngineError::UnknownEngine("Codex"))));
    }

    #[test]
    fn lists_models_in_registration_order() {
        let registry =
            EngineRegistry::new().with(fake(EngineKind::Codex, "gpt")).with(fake(EngineKind::Native, "opus"));
        let ids: Vec<String> = registry.models().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["gpt", "opus"]);
        assert_eq!(registry.kinds(), vec![EngineKind::Codex, EngineKind::Native]);
    }

    #[test]
    fn infos_list_adapters_then_planned_engines() {
        let planned = vec![
            PlannedEngine { kind: EngineKind::Codex, installed: true },
            PlannedEngine { kind: EngineKind::Native, installed: false },
        ];
        let registry = EngineRegistry::new().with(fake(EngineKind::Native, "opus")).with_planned(planned);
        let infos = registry.infos();
        assert_eq!(infos.iter().map(|i| i.kind).collect::<Vec<_>>(), vec![EngineKind::Native, EngineKind::Codex]);
        assert!(infos[0].implemented && infos[0].installed);
        assert!(!infos[1].implemented && infos[1].installed);
        assert!(registry.get(EngineKind::Codex).is_none());
    }

    #[test]
    fn registering_same_kind_replaces() {
        let registry = EngineRegistry::new().with(fake(EngineKind::Codex, "old")).with(fake(EngineKind::Codex, "new"));
        let ids: Vec<String> = registry.models().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["new"]);
    }
}
