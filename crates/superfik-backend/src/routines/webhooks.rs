//! Webhook tokens and call limits (ADR 0012). Each webhook's token is a
//! secret of its owner, kept in the [`SecretStore`] under the webhook's id:
//! never in the routine book, a prompt, a log or a session record. Only the
//! owner sees it, when they ask (`RevealWebhook`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::Engine;
use superfik_core::routine::{WebhookId, WebhookLimit, WebhookToken};
use superfik_core::{ConnectionId, Principal};

use crate::plugins::{SecretStore, Secrets};

const KEY: &str = "webhook_token";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Unknown,
    TooLarge,
    TooMany,
}

#[derive(Clone)]
pub struct Webhooks {
    secrets: Arc<dyn SecretStore>,
    limits: Arc<Mutex<HashMap<WebhookId, WebhookLimit>>>,
}

impl std::fmt::Debug for Webhooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Webhooks").finish_non_exhaustive()
    }
}

fn slot(hook: WebhookId) -> ConnectionId {
    ConnectionId::from_uuid(hook.uuid())
}

impl Webhooks {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        Self { secrets, limits: Arc::default() }
    }

    pub fn token(&self, owner: &Principal, hook: WebhookId) -> Result<WebhookToken, String> {
        if let Some(token) = self.kept(owner, hook) {
            return Ok(token);
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let secrets: Secrets = [(KEY.to_owned(), token.clone())].into();
        self.secrets.set(owner, slot(hook), &secrets)?;
        Ok(WebhookToken(token))
    }

    fn kept(&self, owner: &Principal, hook: WebhookId) -> Option<WebhookToken> {
        self.secrets.get(owner, slot(hook))?.remove(KEY).map(WebhookToken)
    }

    pub fn forget(&self, owner: &Principal, gone: impl IntoIterator<Item = WebhookId>) {
        let mut limits = self.limits.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for hook in gone {
            self.secrets.delete(owner, slot(hook));
            limits.remove(&hook);
        }
    }

    pub fn admit(
        &self,
        owner: &Principal,
        hook: WebhookId,
        token: Option<&str>,
        size: usize,
        now_ms: u64,
    ) -> Result<(), Refusal> {
        let kept = self.kept(owner, hook).ok_or(Refusal::Unknown)?;
        if !token.is_some_and(|token| kept.matches(token)) {
            return Err(Refusal::Unknown);
        }
        if size > superfik_core::routine::MAX_WEBHOOK_BYTES {
            return Err(Refusal::TooLarge);
        }
        let mut limits = self.limits.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if limits.entry(hook).or_default().admit(now_ms) { Ok(()) } else { Err(Refusal::TooMany) }
    }
}
