//! Calls from the network that run on the caller's thread: routine
//! webhooks and providers' calls to channel accounts.

use super::Workers;

impl Workers {
    pub fn receive_webhook(
        &self,
        hook: superfik_core::routine::WebhookId,
        token: Option<&str>,
        body: &[u8],
        now_ms: u64,
    ) -> Result<(), crate::routines::Refusal> {
        self.handler.receive_webhook(hook, token, body, now_ms)
    }

    pub fn receive_channel_hook(
        &self,
        connection: superfik_core::ConnectionId,
        request: &superfik_channels::HookRequest,
    ) -> superfik_channels::HookAnswer {
        self.handler.receive_channel_hook(connection, request)
    }
}
