//! Delegated approvals (`superfik_core::delegation`), answered by the
//! organization's server: a member hands over what their agent asks, the
//! approvers see every ask waiting, and an answer goes back to whoever
//! handed it over, whose desktop resumes the run. Kept in memory: an ask
//! lives no longer than the run on the member's desktop.

use std::sync::Mutex;

use superfik_core::delegation::{self, DelegatedAsk};
use superfik_core::{Member, Principal};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent};

use crate::Organization;

const WAITING_PER_MEMBER: usize = 20;

#[derive(Debug, Default)]
pub struct Delegations {
    waiting: Mutex<Vec<DelegatedAsk>>,
}

impl Delegations {
    fn waiting(&self) -> std::sync::MutexGuard<'_, Vec<DelegatedAsk>> {
        self.waiting.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub fn answer(
    org: &Organization,
    delegations: &Delegations,
    by: &Principal,
    command: &ClientCommand,
) -> Option<Vec<Addressed>> {
    let to_sender = |event| Addressed { to: Recipient::Principal(by.clone()), event };
    let refused = || Some(vec![to_sender(ServerEvent::error("not allowed"))]);
    let concerned = matches!(
        command,
        ClientCommand::DelegateAsk { .. } | ClientCommand::ListDelegated | ClientCommand::AnswerDelegated { .. }
    );
    if !concerned {
        return None;
    }
    let Principal::Member(member) = by else { return refused() };
    let Some(role) = org.directory.role(member) else { return refused() };
    match command {
        ClientCommand::DelegateAsk { run, ask, session } => {
            if !delegation::delegable(ask) {
                return Some(vec![to_sender(ServerEvent::error("only permission asks can be handed over"))]);
            }
            let requester_name = name_of(org, member).unwrap_or_default();
            let mut waiting = delegations.waiting();
            waiting.retain(|kept| !(kept.requester == member.user && kept.run == *run && kept.ask.id == ask.id));
            if waiting.iter().filter(|kept| kept.requester == member.user).count() >= WAITING_PER_MEMBER {
                return Some(vec![to_sender(ServerEvent::error("too many asks are waiting for an approver"))]);
            }
            waiting.push(DelegatedAsk {
                run: *run,
                ask: ask.clone(),
                requester: member.user,
                requester_name,
                session: session.clone(),
            });
            Some(to_approvers(org, member, &waiting))
        }
        ClientCommand::ListDelegated => {
            let waiting = if delegation::approves(role) { delegations.waiting().clone() } else { Vec::new() };
            Some(vec![to_sender(ServerEvent::Delegated(waiting))])
        }
        ClientCommand::AnswerDelegated { run, ask, decision } => {
            let mut waiting = delegations.waiting();
            let Some(at) = waiting.iter().position(|kept| kept.run == *run && &kept.ask.id == ask) else {
                return Some(Vec::new());
            };
            if !delegation::may_answer(&waiting[at], member.user, role) {
                return refused();
            }
            let answered = waiting.remove(at);
            let requester = Member { user: answered.requester, org: member.org };
            let mut events = to_approvers(org, member, &waiting);
            events.push(Addressed {
                to: Recipient::Principal(Principal::Member(requester)),
                event: ServerEvent::DelegatedAnswered {
                    run: answered.run,
                    ask: answered.ask.id,
                    decision: decision.clone(),
                    by: name_of(org, member).unwrap_or_default(),
                },
            });
            Some(events)
        }
        _ => None,
    }
}

fn name_of(org: &Organization, member: &Member) -> Option<String> {
    let accounts = org.directory.members().ok()?;
    accounts.into_iter().find(|account| account.user == member.user).map(|account| account.name)
}

fn to_approvers(org: &Organization, member: &Member, waiting: &[DelegatedAsk]) -> Vec<Addressed> {
    org.directory
        .members()
        .unwrap_or_default()
        .into_iter()
        .filter(|account| delegation::approves(account.role))
        .map(|account| Addressed {
            to: Recipient::Principal(Principal::Member(Member { user: account.user, org: member.org })),
            event: ServerEvent::Delegated(waiting.to_vec()),
        })
        .collect()
}
