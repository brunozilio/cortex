//! Delegated approvals: a member hands what their agent asks to the
//! organization's approvers, so someone else can allow it (a tech lead
//! allowing a deploy). Only permission asks are handed over; questions
//! need the person who started the run.

use serde::{Deserialize, Serialize};

use crate::access::OrgRole;
use crate::{AgentAsk, AskKind, RunId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedAsk {
    pub run: RunId,
    pub ask: AgentAsk,
    pub requester: UserId,
    pub requester_name: String,
    pub session: Option<String>,
}

pub fn delegable(ask: &AgentAsk) -> bool {
    matches!(ask.kind, AskKind::Permission { .. })
}

pub fn approves(role: OrgRole) -> bool {
    crate::access::administers(role)
}

pub fn may_answer(delegated: &DelegatedAsk, by: UserId, role: OrgRole) -> bool {
    by == delegated.requester || approves(role)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AskId, Question};

    fn deploy(requester: UserId) -> DelegatedAsk {
        let kind = AskKind::Permission { tool: "Bash".into(), subject: "cargo publish".into(), rememberable: false };
        DelegatedAsk {
            run: RunId::new(),
            ask: AgentAsk { id: AskId("a1".into()), kind },
            requester,
            requester_name: "Ana".into(),
            session: None,
        }
    }

    #[test]
    fn only_permission_asks_are_handed_over() {
        assert!(delegable(&deploy(UserId::new()).ask));
        let question = Question { header: "Auth".into(), text: "Which?".into(), options: vec![], multi: false };
        assert!(!delegable(&AgentAsk { id: AskId("q".into()), kind: AskKind::Questions(vec![question]) }));
    }

    #[test]
    fn approvers_and_the_requester_may_answer() {
        let (ana, bia) = (UserId::new(), UserId::new());
        let ask = deploy(ana);
        assert!(may_answer(&ask, ana, OrgRole::Member), "takes it back");
        assert!(may_answer(&ask, bia, OrgRole::Admin));
        assert!(may_answer(&ask, bia, OrgRole::Owner));
        assert!(!may_answer(&ask, bia, OrgRole::Member));
        assert!(!may_answer(&ask, bia, OrgRole::Guest));
    }
}
