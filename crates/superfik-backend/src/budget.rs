//! What runs started without a member may spend. A routine's firing has no
//! app to refuse it or report its cost, so the server does both through
//! this seam: an organization's server holds firings to its policy and
//! counts their cost against the owner's month; this computer's own server
//! spends without limit.

use superfik_core::{Principal, PromptRequest};

pub trait Budget: Send + Sync {
    fn authoritative(&self) -> bool {
        false
    }
    fn admit(&self, owner: &Principal, request: &PromptRequest) -> Result<(), String>;
    fn spend(&self, owner: &Principal, model: &str, cost_usd: f64);
}

#[derive(Debug, Default)]
pub struct Unbounded;

impl Budget for Unbounded {
    fn admit(&self, _owner: &Principal, _request: &PromptRequest) -> Result<(), String> {
        Ok(())
    }

    fn spend(&self, _owner: &Principal, _model: &str, _cost_usd: f64) {}
}

pub(crate) fn model_guard(
    budget: std::sync::Arc<dyn Budget>,
    owner: &Principal,
    request: &PromptRequest,
) -> superfik_core::credentials::ModelGuard {
    let mut snapshot = request.clone();
    snapshot.credentials = Default::default();
    let owner = owner.clone();
    superfik_core::credentials::ModelGuard::new(move |model| {
        let mut candidate = snapshot.clone();
        candidate.model = model.clone();
        let prefix = format!("{}:", model.provider.plugin_id());
        if snapshot.model.id.starts_with(&prefix) && !candidate.model.id.starts_with(&prefix) {
            candidate.model.id = format!("{prefix}{}", candidate.model.id);
        }
        budget.admit(&owner, &candidate)
    })
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    #[test]
    fn per_call_guard_preserves_owner_and_rechecks_the_selected_models_policy() {
        struct Policy;
        impl Budget for Policy {
            fn admit(&self, owner: &Principal, request: &PromptRequest) -> Result<(), String> {
                assert_eq!(owner, &Principal::LocalOwner);
                if request.model.id == "anthropic:claude-opus-5-5" { Ok(()) } else { Err("model not permitted".into()) }
            }
            fn spend(&self, _: &Principal, _: &str, _: f64) {}
        }
        let model =
            superfik_core::ModelOption::new(superfik_core::EngineKind::Native, "anthropic:claude-opus-5-5", "Opus");
        let request = PromptRequest::plain(std::env::temp_dir(), "test", model.clone());
        let guard = model_guard(std::sync::Arc::new(Policy), &Principal::LocalOwner, &request);
        assert!(guard.check(&superfik_core::ModelOption { id: "claude-opus-5-5".into(), ..model.clone() }).is_ok());
        assert!(guard.check(&superfik_core::ModelOption { id: "claude-haiku-5-5".into(), ..model }).is_err());
    }
}
