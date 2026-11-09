//! What the webhook routes (`crate::hooks`) ask of the server.

use super::Server;

impl Server {
    pub fn receive_webhook(
        &self,
        hook: superfik_core::routine::WebhookId,
        token: Option<&str>,
        body: &[u8],
        now_ms: u64,
    ) -> Result<(), superfik_backend::WebhookRefusal> {
        self.inner.workers.receive_webhook(hook, token, body, now_ms)
    }

    pub fn receive_channel_hook(
        &self,
        connection: superfik_core::ConnectionId,
        request: &superfik_backend::HookRequest,
    ) -> superfik_backend::HookAnswer {
        self.inner.workers.receive_channel_hook(connection, request)
    }
}
