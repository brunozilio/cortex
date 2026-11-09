//! Channel accounts (ADR 0012) as the handler sees them: linking them,
//! what they report, drafts, and the tools a run gets for them.

use superfik_core::{Principal, PromptRequest, RunId};
use superfik_protocol::ServerEvent;

use super::CommandHandler;
use crate::channels::Inbound;

impl CommandHandler {
    pub fn wake_channels(&self) {
        self.sync_channels(&Principal::LocalOwner);
    }

    pub(super) fn sync_channels(&self, owner: &Principal) {
        let accounts = self.services.plugins.channel_accounts(owner);
        self.services.channels.sync(owner, &accounts);
    }

    pub fn channel_report(&self, report: Inbound) {
        let (owner, connection) = (report.owner.clone(), report.connection);
        let observed = self.services.channels.observe(report);
        if let Some(update) = observed.update {
            self.outbox.answering(&owner).send(update);
        }
        if let Some(incoming) = observed.incoming {
            self.fire_for_message(&owner, connection, &incoming, crate::memory::now());
        }
    }

    pub fn receive_channel_hook(
        &self,
        connection: superfik_core::ConnectionId,
        request: &superfik_channels::HookRequest,
    ) -> superfik_channels::HookAnswer {
        self.services.channels.hook(connection, request)
    }

    pub(super) fn list_channels(&self, owner: &Principal) {
        let outbox = self.outbox.answering(owner);
        for event in self.services.channels.list(owner) {
            outbox.send(event);
        }
    }

    pub(super) fn settle_draft(
        &self,
        owner: &Principal,
        connection: superfik_core::ConnectionId,
        draft: u32,
        text: Option<&str>,
    ) {
        let event = match self.services.channels.settle_draft(owner, connection, draft, text) {
            Ok(event) => event,
            Err(problem) => ServerEvent::error(problem),
        };
        self.outbox.answering(owner).send(event);
    }

    pub(super) fn open_channels(&self, owner: &Principal, run: RunId, request: PromptRequest) -> PromptRequest {
        let Some(tools) = self.channel_tools.as_ref().filter(|_| request.command.is_none()) else {
            return request;
        };
        let accounts = self.services.channels.accounts(owner, &request.connections);
        if accounts.is_empty() {
            return request;
        }
        let titles: Vec<String> = accounts.iter().map(|(_, title)| title.clone()).collect();
        let drafts = request.permission != superfik_core::PermissionMode::Bypass;
        let tool = tools.grant(owner, run, request.cwd.clone(), accounts, drafts);
        let mut brief = request.plugins.clone().unwrap_or_default();
        brief.instructions.push(tool.instructions(&titles));
        brief.channels = Some(tool);
        PromptRequest { plugins: Some(brief), ..request }
    }
}
