//! What routine firings spend on an organization's server (ADR 0012): the
//! policy and the owner's budget, checked before a firing starts, and its
//! cost counted when it ends.

use std::sync::Arc;

use superfik_core::Principal;
use superfik_core::policy::month_of;
use superfik_protocol::{Addressed, Recipient, ServerEvent};

use crate::Organization;
use crate::governance::Governance;

pub(crate) struct GovernedBudget {
    pub(crate) org: Arc<Organization>,
    pub(crate) store: Arc<dyn Governance>,
    pub(crate) deliver: Arc<dyn Fn(Addressed) + Send + Sync>,
}

impl GovernedBudget {
    fn month(&self) -> String {
        month_of(i64::try_from(self.org.now()).unwrap_or(i64::MAX))
    }
}

impl superfik_backend::Budget for GovernedBudget {
    fn authoritative(&self) -> bool {
        true
    }
    fn admit(&self, owner: &Principal, request: &superfik_core::PromptRequest) -> Result<(), String> {
        let Principal::Member(member) = owner else { return Ok(()) };
        if self.store.policy().monthly_budget_usd.is_some()
            && request.model.engine == superfik_core::EngineKind::Native
            && superfik_core::billing::rates(request.model.provider, &request.model.id, u64::MAX).is_none()
        {
            return Err("The model has no reviewed tariff; a budgeted run cannot use an unknown cost.".into());
        }
        let spent = self.store.usage(&self.month()).get(&member.user).map_or(0.0, |(usd, _)| *usd);
        self.store
            .policy()
            .check(request, spent)
            .map_err(|violation| format!("the organization's policy does not allow this run: {violation:?}"))
    }

    fn spend(&self, owner: &Principal, _model: &str, cost_usd: f64) {
        let Principal::Member(member) = owner else { return };
        if !cost_usd.is_finite() || cost_usd <= 0.0 {
            return;
        }
        let month = self.month();
        match self.store.add_usage(&month, member.user, cost_usd) {
            Ok(usd) => (self.deliver)(Addressed {
                to: Recipient::Principal(owner.clone()),
                event: ServerEvent::Spent { month, usd },
            }),
            Err(error) => tracing::warn!(%error, "could not count a routine's cost"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use superfik_backend::Budget;
    use superfik_core::policy::Policy;
    use superfik_core::{EngineKind, Member, ModelOption, PromptRequest, UserId};

    use super::*;
    use crate::governance::InMemoryGovernance;

    const NOW: u64 = 1_792_022_400;

    fn budget(limit: Option<f64>) -> (GovernedBudget, Arc<Mutex<Vec<Addressed>>>, Principal) {
        let org = Arc::new(crate::testing::organization(Arc::new(|| NOW)));
        let id = org.directory.org();
        let store = Arc::new(InMemoryGovernance::default());
        store.set_policy(&Policy { monthly_budget_usd: limit, ..Policy::default() }).expect("policy");
        let sent = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&sent);
        let deliver = Arc::new(move |addressed| kept.lock().expect("lock").push(addressed));
        let owner = Principal::Member(Member { user: UserId::new(), org: id });
        (GovernedBudget { org, store, deliver }, sent, owner)
    }

    fn request() -> PromptRequest {
        let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
        PromptRequest::plain("/srv/checkouts/rocket".into(), "Check Sentry", model)
    }

    #[test]
    fn a_firings_cost_counts_against_the_owners_month() {
        let (budget, sent, owner) = budget(None);
        budget.spend(&owner, "claude-opus-5-5", 1.5);
        budget.spend(&owner, "claude-opus-5-5", 0.0);
        let Principal::Member(member) = &owner else { panic!("a member") };
        assert_eq!(budget.store.usage("2026-10").get(&member.user), Some(&(1.5, 1)));
        let sent = sent.lock().expect("lock");
        assert_eq!(sent.len(), 1, "nothing to tell for a free run");
        assert_eq!(sent[0].event, ServerEvent::Spent { month: "2026-10".into(), usd: 1.5 });
    }

    #[test]
    fn a_firing_is_refused_once_the_budget_is_spent() {
        let (budget, _, owner) = budget(Some(2.0));
        assert!(budget.admit(&owner, &request()).is_ok());
        budget.spend(&owner, "claude-opus-5-5", 2.0);
        let why = budget.admit(&owner, &request()).expect_err("refused");
        assert!(why.contains("Budget"), "{why}");
    }

    #[test]
    fn the_local_owner_is_never_held_back() {
        let (budget, sent, _) = budget(Some(0.0));
        assert!(budget.admit(&Principal::LocalOwner, &request()).is_ok());
        budget.spend(&Principal::LocalOwner, "claude-opus-5-5", 3.0);
        assert!(sent.lock().expect("lock").is_empty());
    }
}
