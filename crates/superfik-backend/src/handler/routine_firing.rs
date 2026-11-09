//! Firing routines (ADR 0012): on the clock, and for a message a channel
//! account received that the agent should answer.

use std::sync::Arc;

use superfik_core::routine::{Cause, Incoming};
use superfik_core::{EngineEvent, RunId};

use super::CommandHandler;
use crate::routines::{self, Firing, Trace};

impl CommandHandler {
    pub fn wake_routines(&self, now: i64) {
        self.services.routines.wake(now);
    }

    pub fn fire_due_routines(&self, now: i64) {
        for firing in self.services.routines.due(now) {
            self.fire(&firing, Cause::Schedule, now);
        }
    }

    pub(crate) fn fire_for_message(
        &self,
        owner: &superfik_core::Principal,
        connection: superfik_core::ConnectionId,
        incoming: &Incoming,
        now: i64,
    ) {
        for firing in self.services.routines.listening(owner, connection, &incoming.exchange) {
            self.fire(&firing, Cause::Message(incoming), now);
        }
    }

    pub(super) fn fire(&self, firing: &Firing, cause: Cause<'_>, now: i64) {
        let mut models = self.services.engines.models();
        if let Some(folder) = &self.services.local_models {
            models.extend(superfik_gateway::mlx::downloaded(folder, superfik_core::EngineKind::Native));
        }
        let place = routines::place(firing, &self.services.worktree_root, now);
        let branch = place.as_ref().map(|(branch, _)| branch.clone());
        let Some(request) = routines::request(firing, &models, branch.as_deref(), cause) else {
            tracing::warn!(routine = %firing.routine.name, "no model to run a routine with");
            return;
        };
        let run = RunId::new();
        let outbox = self.outbox.answering(&firing.owner);
        let started = match cause {
            Cause::Schedule => self.services.routines.started(firing, run, branch, now),
            Cause::Message(incoming) => self.services.routines.answering(firing, run, &incoming.exchange, branch, now),
            Cause::Webhook(_) => self.services.routines.hooked_start(firing, run, branch, now),
        };
        if let Some(book) = started {
            outbox.send(book);
        }
        let budget = (Arc::clone(&self.services.budget), request.model.id.clone());
        if let Err(why) = self.services.budget.admit(&firing.owner, &request) {
            let trace = Trace::new(self.services.routines.clone(), firing, None);
            if let Some(book) = trace.observe(run, &EngineEvent::Failed(why), now) {
                outbox.send(book);
            }
            return;
        }
        let workdir = place.as_ref().map(|(_, path)| path.clone());
        let mut trace = Trace::new(self.services.routines.clone(), firing, workdir).spending(budget);
        if let Some((branch, path)) = &place {
            if let Err(error) = self.services.worktrees.add(&firing.project, path, branch) {
                let failed = EngineEvent::Failed(format!("could not prepare a worktree: {error}"));
                if let Some(book) = trace.observe(run, &failed, now) {
                    outbox.send(book);
                }
                return;
            }
            trace = trace.cleaning(Arc::clone(&self.services.worktrees), branch.clone());
        }
        self.submit_traced(&firing.owner, run, &request, Some(Arc::new(trace)));
    }
}
