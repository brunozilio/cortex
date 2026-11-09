//! Delegated approvals in the app (`superfik_core::delegation`): handing
//! the ask on screen to the organization's approvers, resuming the run
//! with their answer, and, for an approver, the asks waiting for one.

use superfik_core::delegation::{self, DelegatedAsk};
use superfik_core::{AskId, Decision, RunId};
use superfik_protocol::ClientCommand;

use super::{Effect, Notice, PendingAsk, Session};

fn command(command: ClientCommand) -> Effect {
    Effect::Command(Box::new(command))
}

impl Session {
    pub fn may_delegate(&self, pending: &PendingAsk) -> bool {
        self.organization.joined() && !pending.delegated && delegation::delegable(&pending.ask)
    }

    pub(super) fn delegate(&mut self) -> Vec<Effect> {
        let Some(pending) = self.pending_ask() else { return Vec::new() };
        if !self.may_delegate(pending) {
            return Vec::new();
        }
        let session = self.conversation().title.clone();
        let Some(pending) = self.conversation_mut().asks.front_mut() else { return Vec::new() };
        pending.delegated = true;
        vec![command(ClientCommand::DelegateAsk { run: pending.run, ask: pending.ask.clone(), session })]
    }

    pub(super) fn delegated_answered(
        &mut self,
        run: RunId,
        ask: &AskId,
        decision: Decision,
        by: String,
    ) -> Vec<Effect> {
        for conversation in &mut self.conversations {
            let Some(at) = conversation.asks.iter().position(|p| p.delegated && p.run == run && &p.ask.id == ask)
            else {
                continue;
            };
            conversation.asks.remove(at);
            self.notice = Some(Notice::Delegated { by, allowed: decision != Decision::Deny });
            return vec![command(ClientCommand::AnswerAsk { run, ask: ask.clone(), decision })];
        }
        Vec::new()
    }

    pub(super) fn handed_over(&self) -> Vec<(RunId, AskId)> {
        let asks = self.conversations.iter().flat_map(|c| &c.asks);
        asks.filter(|pending| pending.delegated).map(|pending| (pending.run, pending.ask.id.clone())).collect()
    }

    pub(super) fn release_handed_over(&self, before: Vec<(RunId, AskId)>) -> Vec<Effect> {
        let still = self.handed_over();
        before
            .into_iter()
            .filter(|handed| !still.contains(handed))
            .map(|(run, ask)| command(ClientCommand::AnswerDelegated { run, ask, decision: Decision::Deny }))
            .collect()
    }

    pub(super) fn delegated_listed(&mut self, waiting: Vec<DelegatedAsk>) {
        let known = &self.organization.delegated;
        if let Some(new) = waiting.iter().find(|ask| !known.iter().any(|k| k.ask.id == ask.ask.id)) {
            self.notice = Some(Notice::ApprovalWaiting(new.requester_name.clone()));
        }
        self.organization.delegated = waiting;
    }

    pub(super) fn answer_delegated(&mut self, index: usize, allow: bool) -> Vec<Effect> {
        let Some(waiting) = self.organization.delegated.get(index) else { return Vec::new() };
        let decision = if allow { Decision::Allow } else { Decision::Deny };
        let (run, ask) = (waiting.run, waiting.ask.id.clone());
        self.organization.delegated.remove(index);
        vec![command(ClientCommand::AnswerDelegated { run, ask, decision })]
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{AgentAsk, AskKind, EngineEvent, Member, OrgId, Principal, UserId};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey, Reply};

    fn deploy() -> AgentAsk {
        let kind = AskKind::Permission { tool: "Bash".into(), subject: "cargo publish".into(), rememberable: false };
        AgentAsk { id: AskId("p1".into()), kind }
    }

    fn asked() -> (Session, RunId) {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some("/repo".into());
        session.apply(Action::Edit(EditKey::Insert("Release it".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        let run = session.conversation().run.as_ref().expect("running").id;
        session.apply(Action::Server(ServerEvent::Run { run, event: EngineEvent::Asked(deploy()) }), 1.0);
        (session, run)
    }

    #[test]
    fn handing_an_ask_over_keeps_it_on_screen_and_sends_it_once() {
        let (mut session, run) = asked();
        let effects = session.apply(Action::Reply(Reply::Delegate), 2.0);
        let sent = commands(&effects);
        assert!(matches!(sent[..], [ClientCommand::DelegateAsk { run: r, session: title, .. }]
            if *r == run && title.as_deref() == Some("Release it")));
        let pending = session.pending_ask().expect("still there");
        assert!(pending.delegated && !session.may_delegate(pending));
        assert!(commands(&session.apply(Action::Reply(Reply::Delegate), 3.0)).is_empty());
    }

    #[test]
    fn alone_nothing_is_handed_over() {
        let mut session = Session::new(Some("/repo".into()));
        session.apply(Action::Edit(EditKey::Insert("Release it".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        let run = session.conversation().run.as_ref().expect("running").id;
        session.apply(Action::Server(ServerEvent::Run { run, event: EngineEvent::Asked(deploy()) }), 1.0);
        assert!(commands(&session.apply(Action::Reply(Reply::Delegate), 2.0)).is_empty());
    }

    #[test]
    fn an_approvers_answer_resumes_the_run() {
        let (mut session, run) = asked();
        session.apply(Action::Reply(Reply::Delegate), 2.0);
        let answered = ServerEvent::DelegatedAnswered {
            run,
            ask: AskId("p1".into()),
            decision: Decision::Allow,
            by: "Olga".into(),
        };
        let effects = session.apply(Action::Server(answered), 3.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::AnswerAsk { decision: Decision::Allow, .. }]));
        assert!(session.pending_ask().is_none());
        assert_eq!(session.notice, Some(Notice::Delegated { by: "Olga".into(), allowed: true }));
    }

    #[test]
    fn answering_here_takes_it_back_from_the_approvers() {
        let (mut session, _) = asked();
        session.apply(Action::Reply(Reply::Delegate), 2.0);
        let effects = session.apply(Action::Reply(Reply::Deny), 3.0);
        let sent = commands(&effects);
        assert!(matches!(sent[..], [ClientCommand::AnswerAsk { .. }, ClientCommand::AnswerDelegated { .. }]));
    }

    #[test]
    fn an_ask_whose_run_ends_leaves_the_approvers_list() {
        let (mut session, run) = asked();
        session.apply(Action::Reply(Reply::Delegate), 2.0);
        let ended = ServerEvent::Run { run, event: EngineEvent::Cancelled };
        let effects = session.apply(Action::Server(ended), 3.0);
        assert!(commands(&effects).iter().any(
            |c| matches!(c, ClientCommand::AnswerDelegated { ask, decision: Decision::Deny, .. } if ask.0 == "p1")
        ));
    }

    #[test]
    fn an_approver_hears_of_a_new_ask_and_answers_it() {
        let mut session = Session::default();
        let waiting = DelegatedAsk {
            run: RunId::new(),
            ask: deploy(),
            requester: UserId::new(),
            requester_name: "Ana".into(),
            session: None,
        };
        session.apply(Action::Server(ServerEvent::Delegated(vec![waiting])), 0.0);
        assert_eq!(session.notice, Some(Notice::ApprovalWaiting("Ana".into())));
        let effects = session.apply(Action::Organization(crate::state::OrgAction::AnswerDelegated(0, true)), 1.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::AnswerDelegated { decision: Decision::Allow, .. }]));
        assert!(session.organization.delegated.is_empty());
    }
}
