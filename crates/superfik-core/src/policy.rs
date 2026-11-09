//! The organization's policy: what its members' agents may use and how
//! much they may spend. Pure; owners and admins set it on the
//! organization's server, which checks every run against it (ADR 0010).

use serde::{Deserialize, Serialize};

use crate::{PermissionMode, PromptRequest};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub max_mode: Option<PermissionMode>,
    #[serde(default)]
    pub monthly_budget_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Violation {
    Model(String),
    Unpriced(String),
    Mode(PermissionMode),
    Budget { spent_usd: f64, budget_usd: f64 },
}

fn latitude(mode: PermissionMode) -> u8 {
    match mode {
        PermissionMode::Plan => 0,
        PermissionMode::Ask => 1,
        PermissionMode::AcceptEdits => 2,
        PermissionMode::Bypass => 3,
    }
}

impl Policy {
    pub fn allows_model(&self, id: &str) -> bool {
        self.models.as_ref().is_none_or(|models| models.iter().any(|m| m == id))
    }

    pub fn allows_mode(&self, mode: PermissionMode) -> bool {
        self.max_mode.is_none_or(|max| latitude(mode) <= latitude(max))
    }

    pub fn cap(&self, mode: PermissionMode) -> PermissionMode {
        match self.max_mode {
            Some(max) if !self.allows_mode(mode) => max,
            _ => mode,
        }
    }

    pub fn check(&self, request: &PromptRequest, spent_usd: f64) -> Result<(), Violation> {
        self.check_model(&request.model, request.permission, spent_usd)
    }

    pub fn check_model(
        &self,
        model: &crate::ModelOption,
        mode: PermissionMode,
        spent_usd: f64,
    ) -> Result<(), Violation> {
        self.check_run(&model.id, mode, spent_usd)?;
        if self.monthly_budget_usd.is_some()
            && model.engine == crate::EngineKind::Native
            && crate::billing::rates(model.provider, &model.id, u64::MAX).is_none()
        {
            return Err(Violation::Unpriced(model.id.clone()));
        }
        Ok(())
    }

    pub fn check_run(&self, model: &str, mode: PermissionMode, spent_usd: f64) -> Result<(), Violation> {
        if !self.allows_model(model) {
            return Err(Violation::Model(model.to_owned()));
        }
        if !self.allows_mode(mode) {
            return Err(Violation::Mode(mode));
        }
        match self.monthly_budget_usd {
            Some(budget_usd) if spent_usd >= budget_usd => Err(Violation::Budget { spent_usd, budget_usd }),
            _ => Ok(()),
        }
    }
}

pub fn month_of(unix: i64) -> String {
    let (year, month, _) = crate::days::civil(unix.div_euclid(86_400));
    format!("{year:04}-{month:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Effort, EngineKind, ModelOption, WorkspaceId};

    fn request(model: &str, permission: PermissionMode) -> PromptRequest {
        PromptRequest {
            workspace: WorkspaceId::new(),
            cwd: "/repo".into(),
            prompt: "Ship it".into(),
            model: ModelOption::new(EngineKind::Native, model, model),
            effort: Effort::High,
            repositories: vec![],
            skills: vec![],
            disabled_skills: vec![],
            references: vec![],
            notes: vec![],
            resume: None,
            command: None,
            handoff: None,
            memory: None,
            permission,
            output_style: None,
            safety_check: true,
            connections: vec![],
            plugins: None,
            browser: None,
            credentials: Default::default(),
        }
    }

    #[test]
    fn no_policy_allows_everything() {
        assert_eq!(Policy::default().check(&request("claude-opus-5-5", PermissionMode::Bypass), 1e9), Ok(()));
    }

    #[test]
    fn models_outside_the_list_are_refused() {
        let policy = Policy { models: Some(vec!["claude-sonnet-5-5".into()]), ..Policy::default() };
        assert_eq!(policy.check(&request("claude-sonnet-5-5", PermissionMode::Ask), 0.0), Ok(()));
        assert_eq!(
            policy.check(&request("claude-opus-5-5", PermissionMode::Ask), 0.0),
            Err(Violation::Model("claude-opus-5-5".into()))
        );
    }

    #[test]
    fn a_mode_beyond_the_maximum_is_refused_and_capped() {
        let policy = Policy { max_mode: Some(PermissionMode::AcceptEdits), ..Policy::default() };
        assert!(policy.allows_mode(PermissionMode::Plan) && policy.allows_mode(PermissionMode::Ask));
        assert_eq!(
            policy.check(&request("m", PermissionMode::Bypass), 0.0),
            Err(Violation::Mode(PermissionMode::Bypass))
        );
        assert_eq!(policy.cap(PermissionMode::Bypass), PermissionMode::AcceptEdits);
        assert_eq!(policy.cap(PermissionMode::Plan), PermissionMode::Plan);
    }

    #[test]
    fn the_budget_stops_runs_once_spent() {
        let policy = Policy { monthly_budget_usd: Some(50.0), ..Policy::default() };
        assert_eq!(policy.check(&request("claude-opus-5-5", PermissionMode::Ask), 49.9), Ok(()));
        assert_eq!(
            policy.check(&request("claude-opus-5-5", PermissionMode::Ask), 50.0),
            Err(Violation::Budget { spent_usd: 50.0, budget_usd: 50.0 })
        );
    }

    #[test]
    fn months_are_utc_calendar_months() {
        assert_eq!(month_of(0), "1970-01");
        assert_eq!(month_of(1_792_022_400), "2026-10", "15 October 2026");
    }
}
#[cfg(test)]
mod native_tariff_tests {
    use super::*;
    #[test]
    fn unknown_native_tariff_is_refused_only_when_a_budget_must_be_enforced() {
        let model = crate::ModelOption::new(crate::EngineKind::Native, "unreviewed", "Unreviewed");
        let policy = Policy { monthly_budget_usd: Some(10.0), ..Default::default() };
        assert_eq!(policy.check_model(&model, PermissionMode::Ask, 0.0), Err(Violation::Unpriced("unreviewed".into())));
        assert!(Policy::default().check_model(&model, PermissionMode::Ask, 0.0).is_ok());
        let local = model.served_by(crate::ModelProvider::Local);
        assert!(policy.check_model(&local, PermissionMode::Ask, 0.0).is_ok());
    }
}
