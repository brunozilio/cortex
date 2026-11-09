//! The organization's policy in the app (`superfik_core::policy`): it is
//! kept, handed to this device's runner and checked before a prompt goes
//! out; what each run cost is reported for budgets. Owners and admins
//! change it in Settings › Server (`Setting::Policy`).

use superfik_core::policy::{Policy, Violation};
use superfik_core::{PermissionMode, RunId};
use superfik_protocol::{ClientCommand, MemberUsage};

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyChoice {
    Mode(Option<PermissionMode>),
    Model(usize, bool),
    Budget(Option<u32>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyRefusal {
    Model(String),
    Unpriced(String),
    Mode(PermissionMode),
    Budget(u32),
}

fn command(command: ClientCommand) -> Effect {
    Effect::Command(Box::new(command))
}

impl Session {
    pub(super) fn policy_arrived(&mut self, policy: Policy) -> Vec<Effect> {
        for conversation in &mut self.conversations {
            conversation.permission = policy.cap(conversation.permission);
        }
        self.organization.policy = policy;
        Vec::new()
    }

    pub(super) fn spent_arrived(&mut self, usd: f64) -> Vec<Effect> {
        self.organization.spent_usd = usd;
        Vec::new()
    }

    pub(super) fn usage_arrived(&mut self, usage: Vec<MemberUsage>) {
        self.organization.usage = usage;
    }

    pub(super) fn policy_refusal(&self) -> Option<PolicyRefusal> {
        let (model, mode) = (self.model(), self.conversation().permission);
        match self.organization.policy.check_model(model, mode, self.organization.spent_usd) {
            Ok(()) => None,
            Err(Violation::Unpriced(model)) => Some(PolicyRefusal::Unpriced(model)),
            Err(Violation::Model(model)) => Some(PolicyRefusal::Model(model)),
            Err(Violation::Mode(mode)) => Some(PolicyRefusal::Mode(mode)),
            Err(Violation::Budget { budget_usd, .. }) => Some(PolicyRefusal::Budget(budget_usd as u32)),
        }
    }

    pub(super) fn report_usage(&self, run: RunId, cost_usd: Option<f64>) -> Vec<Effect> {
        let owner = self.conversations.iter().find(|c| c.owns_run(run));
        match (cost_usd, owner) {
            (Some(cost_usd), Some(conversation)) if self.organization.joined() => {
                let model = self.models.get(conversation.model).map(|m| m.id.clone()).unwrap_or_default();
                vec![command(ClientCommand::ReportUsage { run, model, cost_usd })]
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn choose_policy(&mut self, choice: PolicyChoice) -> Vec<Effect> {
        if !self.manages_organization() {
            return Vec::new();
        }
        let mut policy = self.organization.policy.clone();
        match choice {
            PolicyChoice::Mode(mode) => policy.max_mode = mode,
            PolicyChoice::Budget(dollars) => policy.monthly_budget_usd = dollars.map(f64::from),
            PolicyChoice::Model(index, allowed) => {
                let Some(id) = self.models.get(index).map(|m| m.id.clone()) else { return Vec::new() };
                let all: Vec<String> = self.models.iter().map(|m| m.id.clone()).collect();
                let mut models = policy.models.take().unwrap_or_else(|| all.clone());
                models.retain(|m| *m != id);
                if allowed {
                    models.push(id);
                }
                policy.models = (!all.iter().all(|m| models.contains(m))).then_some(models);
            }
        }
        vec![command(ClientCommand::SetPolicy { policy })]
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::access::{Colleague, OrgRole};
    use superfik_core::{EngineEvent, EngineKind, Member, ModelOption, OrgId, Principal, UserId};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey, Notice, Setting};

    fn member(role: OrgRole) -> Session {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some("/repo".into());
        let colleague = Colleague { user: me.user, name: "olga".into(), role, pending: false };
        session.apply(Action::Server(ServerEvent::Members(vec![colleague])), 0.0);
        let models = vec![
            ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
            ModelOption::new(EngineKind::Native, "claude-sonnet-5-5", "Sonnet 5.5"),
        ];
        session.apply(Action::Server(ServerEvent::Models(models)), 0.0);
        session
    }

    fn policy(session: &mut Session, policy: Policy) -> Vec<Effect> {
        session.apply(Action::Server(ServerEvent::Policy(policy)), 0.0)
    }

    #[test]
    fn joining_asks_for_the_policy_and_modes_come_down_to_it() {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        let effects = session.apply(Action::Server(welcome), 0.0);
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::GetPolicy)));
        let capped = Policy { max_mode: Some(PermissionMode::Ask), ..Policy::default() };
        let effects = policy(&mut session, capped);
        assert!(commands(&effects).is_empty(), "the server enforces it; nothing to hand over");
        assert_eq!(session.conversation().permission, PermissionMode::Ask, "came down to the maximum");
    }

    #[test]
    fn a_prompt_the_policy_forbids_is_not_sent() {
        let mut session = member(OrgRole::Member);
        policy(&mut session, Policy { models: Some(vec!["claude-sonnet-5-5".into()]), ..Policy::default() });
        session.apply(Action::Edit(EditKey::Insert("Ship it".into())), 0.0);
        let effects = session.apply(Action::Submit, 1.0);
        assert!(!commands(&effects).iter().any(|c| matches!(c, ClientCommand::SubmitPrompt { .. })));
        assert_eq!(session.notice, Some(Notice::Policy(PolicyRefusal::Model("claude-opus-5-5".into()))));
        assert!(!session.is_model_selectable(0) && session.is_model_selectable(1));
    }

    #[test]
    fn over_budget_nothing_is_sent() {
        let mut session = member(OrgRole::Member);
        policy(&mut session, Policy { monthly_budget_usd: Some(50.0), ..Policy::default() });
        session.apply(Action::Server(ServerEvent::Spent { month: "2026-10".into(), usd: 50.0 }), 0.0);
        session.apply(Action::Edit(EditKey::Insert("Ship it".into())), 0.0);
        session.apply(Action::Submit, 1.0);
        assert_eq!(session.notice, Some(Notice::Policy(PolicyRefusal::Budget(50))));
    }

    #[test]
    fn native_usage_is_reported_once_and_completion_does_not_count_it_again() {
        let mut session = member(OrgRole::Member);
        session.apply(Action::Edit(EditKey::Insert("Ship it".into())), 0.0);
        session.apply(Action::Submit, 1.0);
        let run = session.conversation().run.as_ref().expect("running").id;
        let event = EngineEvent::Usage(superfik_core::billing::ModelUsage {
            accounted_by_server: false,
            model: "claude-opus-5-5".into(),
            input: 10,
            output: 5,
            cache_read: 0,
            cache_write: 0,
            estimated_cost_usd: Some(0.5),
            basis: "fixture".into(),
            price_date: "2026-11-08".into(),
        });
        let effects = session.apply(Action::Server(ServerEvent::Run { run, event }), 2.0);
        assert!(commands(&effects).iter().any(
            |command| matches!(command,ClientCommand::ReportUsage{cost_usd,..} if (*cost_usd-0.5).abs()<f64::EPSILON)
        ));
        let effects = session.apply(
            Action::Server(ServerEvent::Run {
                run,
                event: EngineEvent::Completed { duration_ms: 5, cost_usd: Some(0.5) },
            }),
            3.0,
        );
        assert!(!commands(&effects).iter().any(|command| matches!(command, ClientCommand::ReportUsage { .. })));
    }

    #[test]
    fn an_admin_changes_the_policy_line_by_line() {
        let mut session = member(OrgRole::Admin);
        let choose = |choice| Action::ChooseSetting(Setting::Policy(choice));
        let effects = session.apply(choose(PolicyChoice::Model(0, false)), 0.0);
        let sent = commands(&effects);
        let [ClientCommand::SetPolicy { policy }] = &sent[..] else { panic!("not sent") };
        assert_eq!(policy.models, Some(vec!["claude-sonnet-5-5".to_owned()]));
        let effects = session.apply(choose(PolicyChoice::Budget(Some(100))), 0.0);
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::SetPolicy { policy }] if policy.monthly_budget_usd == Some(100.0))
        );
        let mut member = member(OrgRole::Member);
        assert!(commands(&member.apply(choose(PolicyChoice::Mode(None)), 0.0)).is_empty());
    }
}
