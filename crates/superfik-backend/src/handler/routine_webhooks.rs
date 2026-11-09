//! Routine webhooks (ADR 0012): giving the owner its address and token,
//! forgetting tokens of webhooks that are gone, and taking a call.

use std::path::Path;

use superfik_core::Principal;
use superfik_core::routine::{Cause, RoutineEdit, RoutineId, WebhookId};
use superfik_protocol::ServerEvent;

use super::CommandHandler;
use crate::memory;
use crate::routines::Refusal;

impl CommandHandler {
    pub(super) fn edit_routines(&self, owner: &Principal, project: &Path, edits: &[RoutineEdit]) -> ServerEvent {
        let routines = &self.services.routines;
        routines.load(owner, project);
        let before = routines.webhooks(owner, project);
        let answer = routines.edit(owner, project, edits, memory::now());
        let after = routines.webhooks(owner, project);
        let gone = before.into_iter().filter(|hook| !after.contains(hook)).map(|(_, hook)| hook);
        self.webhooks.forget(owner, gone);
        answer
    }

    pub(super) fn reveal_webhook(&self, owner: &Principal, project: &Path, routine: RoutineId) -> ServerEvent {
        self.services.routines.load(owner, project);
        let found = self.services.routines.webhooks(owner, project).into_iter().find(|(id, _)| *id == routine);
        let Some((_, hook)) = found else {
            return ServerEvent::error("this routine has no webhook".to_owned());
        };
        match self.webhooks.token(owner, hook) {
            Ok(token) => ServerEvent::RoutineWebhook { path: project.to_path_buf(), routine, hook, token },
            Err(error) => ServerEvent::error(format!("could not keep the webhook's token: {error}")),
        }
    }

    pub fn receive_webhook(
        &self,
        hook: WebhookId,
        token: Option<&str>,
        body: &[u8],
        now_ms: u64,
    ) -> Result<(), Refusal> {
        let firing = self.services.routines.hooked(hook).ok_or(Refusal::Unknown)?;
        self.webhooks.admit(&firing.owner, hook, token, body.len(), now_ms)?;
        let body = String::from_utf8_lossy(body);
        let now = i64::try_from(now_ms / 1_000).unwrap_or(i64::MAX);
        self.fire(&firing, Cause::Webhook(&body), now);
        Ok(())
    }
}
