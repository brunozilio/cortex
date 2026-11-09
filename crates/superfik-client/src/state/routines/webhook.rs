//! A routine's webhook in the client: the switch in its form, and copying
//! a ready `curl` command for the service that will call it. The token is
//! asked for only when copied, and goes straight to the clipboard: the
//! session never keeps it.

use std::path::PathBuf;

use superfik_core::routine::{RoutineId, WebhookId, WebhookToken};
use superfik_protocol::ClientCommand;

use super::super::{Effect, Session};

pub fn curl(hook: WebhookId, token: &WebhookToken) -> String {
    format!(
        "curl -X POST \"$SUPERFIK_SERVER/hooks/{hook}\" -H \"Authorization: Bearer {}\" --data-binary @payload.json",
        token.0
    )
}

impl Session {
    pub(super) fn copy_webhook(&mut self, routine: RoutineId) -> Vec<Effect> {
        let Some(path) = self.root.clone() else { return Vec::new() };
        if self.routines.book.get(routine).and_then(|r| r.webhook).is_none() {
            return Vec::new();
        }
        self.routines.copying = Some(routine);
        vec![Effect::Command(Box::new(ClientCommand::RevealWebhook { path, routine }))]
    }

    pub(in crate::state) fn webhook_arrived(
        &mut self,
        path: PathBuf,
        routine: RoutineId,
        hook: WebhookId,
        token: WebhookToken,
        now: f64,
    ) -> Vec<Effect> {
        if self.root.as_ref() != Some(&path) || self.routines.copying != Some(routine) {
            return Vec::new();
        }
        self.routines.copying = None;
        self.routines.copied = Some((routine, now));
        vec![Effect::CopyText(curl(hook, &token))]
    }
}
