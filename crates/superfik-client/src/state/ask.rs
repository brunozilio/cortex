//! What a running agent asked the user, and the answers the user gives.
//!
//! Asks queue per conversation, oldest first; the active conversation shows
//! its first one in place of the composer. Each answer goes back to the run
//! as a `ClientCommand::AnswerAsk`.

use std::collections::VecDeque;

use superfik_core::{AgentAsk, AskId, AskKind, Decision, PermissionMode, Question, RunId};
use superfik_protocol::ClientCommand;

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Allow,
    AllowAlways,
    Deny,
    Back,
    Delegate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingAsk {
    pub run: RunId,
    pub ask: AgentAsk,
    pub question: usize,
    pub picks: Vec<Vec<usize>>,
    pub cursor: usize,
    pub arrived_at: f64,
    pub delegated: bool,
}

impl PendingAsk {
    pub fn new(run: RunId, ask: AgentAsk, now: f64) -> Self {
        let questions = match &ask.kind {
            AskKind::Questions(questions) => questions.len(),
            AskKind::Permission { .. } => 0,
        };
        Self { run, ask, question: 0, picks: vec![Vec::new(); questions], cursor: 0, arrived_at: now, delegated: false }
    }

    pub fn current_question(&self) -> Option<&Question> {
        match &self.ask.kind {
            AskKind::Questions(questions) => questions.get(self.question),
            AskKind::Permission { .. } => None,
        }
    }

    pub fn replies(&self) -> Vec<Reply> {
        match self.ask.kind {
            AskKind::Permission { rememberable: true, .. } => vec![Reply::Allow, Reply::AllowAlways, Reply::Deny],
            _ => vec![Reply::Allow, Reply::Deny],
        }
    }

    fn rows(&self) -> usize {
        self.current_question().map_or_else(|| self.replies().len(), |q| q.options.len())
    }

    pub fn picked(&self, option: usize) -> bool {
        self.picks.get(self.question).is_some_and(|picked| picked.contains(&option))
    }
}

impl Session {
    pub fn pending_ask(&self) -> Option<&PendingAsk> {
        self.conversation().asks.front()
    }

    pub(super) fn reply(&mut self, reply: Reply) -> Vec<Effect> {
        if reply == Reply::Delegate {
            return self.delegate();
        }
        let Some(pending) = self.conversation_mut().asks.front_mut() else { return Vec::new() };
        if reply == Reply::Back {
            if pending.current_question().is_some() && pending.question > 0 {
                pending.question -= 1;
                pending.cursor = pending.picks[pending.question].first().copied().unwrap_or(0);
            }
            return Vec::new();
        }
        let decision = match (&pending.ask.kind, reply) {
            (_, Reply::Deny) => Decision::Deny,
            (AskKind::Permission { rememberable: true, .. }, Reply::AllowAlways) => Decision::AllowAlways,
            (AskKind::Permission { .. }, _) => Decision::Allow,
            (AskKind::Questions(questions), _) => {
                if pending.picks[pending.question].is_empty() {
                    pending.picks[pending.question].push(pending.cursor);
                }
                if pending.question + 1 < questions.len() {
                    pending.question += 1;
                    pending.cursor = 0;
                    return Vec::new();
                }
                Decision::Answer(pending.picks.clone())
            }
        };
        self.answer(decision)
    }

    pub(super) fn pick(&mut self, index: usize) -> Vec<Effect> {
        let Some(pending) = self.conversation_mut().asks.front_mut() else { return Vec::new() };
        let Some(multi) = pending.current_question().map(|q| q.multi) else {
            return match pending.replies().get(index) {
                Some(reply) => self.reply(*reply),
                None => Vec::new(),
            };
        };
        if index >= pending.rows() {
            return Vec::new();
        }
        pending.cursor = index;
        if !multi {
            pending.picks[pending.question] = vec![index];
            return self.reply(Reply::Allow);
        }
        let picked = &mut pending.picks[pending.question];
        match picked.iter().position(|i| *i == index) {
            Some(at) => {
                picked.remove(at);
            }
            None => picked.push(index),
        }
        Vec::new()
    }

    pub(super) fn move_cursor(&mut self, delta: i32) -> Vec<Effect> {
        if let Some(pending) = self.conversation_mut().asks.front_mut() {
            let last = pending.rows().saturating_sub(1) as i64;
            pending.cursor = (pending.cursor as i64 + i64::from(delta)).clamp(0, last) as usize;
        }
        Vec::new()
    }

    pub(super) fn set_permission_mode(&mut self, mode: PermissionMode) -> Vec<Effect> {
        let mode = self.organization.policy.cap(mode);
        self.conversation_mut().permission = mode;
        self.popover = None;
        match &self.conversation().run {
            Some(run) => vec![Effect::Command(Box::new(ClientCommand::SetPermissionMode { run: run.id, mode }))],
            None => Vec::new(),
        }
    }

    fn answer(&mut self, decision: Decision) -> Vec<Effect> {
        let Some(pending) = self.conversation_mut().asks.pop_front() else { return Vec::new() };
        let taken_back = pending.delegated.then(|| ClientCommand::AnswerDelegated {
            run: pending.run,
            ask: pending.ask.id.clone(),
            decision: decision.clone(),
        });
        let command = ClientCommand::AnswerAsk { run: pending.run, ask: pending.ask.id, decision };
        std::iter::once(command).chain(taken_back).map(|command| Effect::Command(Box::new(command))).collect()
    }
}

pub(super) fn enqueue(asks: &mut VecDeque<PendingAsk>, run: RunId, ask: &AgentAsk, now: f64) {
    if !asks.iter().any(|pending| pending.ask.id == ask.id) {
        asks.push_back(PendingAsk::new(run, ask.clone(), now));
    }
}

pub(super) fn withdraw(asks: &mut VecDeque<PendingAsk>, id: &AskId) {
    asks.retain(|pending| &pending.ask.id != id);
}
