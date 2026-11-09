//! What reaches a link from outside: its own reports, and providers' calls
//! to its webhook.

use superfik_channels::{ChannelEvent, CloseReason, HookAnswer, HookRequest};
use superfik_core::ConnectionId;
use superfik_core::channel::{LinkStatus, PAUSE_MS, Verdict, is_news};
use superfik_core::routine::Incoming;

use super::{Channels, Inbound, Observed, lock};

pub fn hook_path(connection: ConnectionId) -> String {
    format!("/hooks/channels/{connection}")
}

impl Channels {
    pub fn observe(&self, inbound: Inbound) -> Observed {
        let Inbound { owner, connection, event } = inbound;
        let status = match event {
            ChannelEvent::Qr { code, .. } => LinkStatus::Pairing { qr: code },
            ChannelEvent::Linked { account } => {
                let takes = lock(&self.inner.links).get(&connection).is_some_and(|l| l.kind.takes_webhooks());
                LinkStatus::Linked { account, hook: takes.then(|| hook_path(connection)) }
            }
            ChannelEvent::Closed(CloseReason::LoggedOut) => LinkStatus::LoggedOut,
            ChannelEvent::Closed(CloseReason::Failed(reason)) => LinkStatus::Failed { reason },
            ChannelEvent::File { message, media, bytes } => {
                return self.keep_file(connection, &message, &media, &bytes.0);
            }
            ChannelEvent::Message { message, delivery } => {
                let message = self.with_kept_file(connection, message);
                let title = lock(&self.inner.links).get(&connection).map(|l| l.connection.title());
                let mut verdict = None;
                let (changed, last_seen) = self.change(&owner, connection, |book| {
                    verdict = book.receive(message.clone(), PAUSE_MS);
                    verdict.is_some()
                });
                let news = is_news(delivery, message.at, last_seen);
                let incoming = (verdict == Some(Verdict::Answer) && news && changed).then(|| Incoming {
                    channel: title.unwrap_or_default(),
                    exchange: message.exchange.clone(),
                    from: message.sender_name.clone(),
                    text: message.text.clone(),
                });
                return Observed { update: None, incoming };
            }
        };
        let known = {
            let mut links = lock(&self.inner.links);
            links.get_mut(&connection).map(|link| link.status = status).is_some()
        };
        Observed { update: known.then(|| self.update(&owner, connection)), incoming: None }
    }

    pub fn hook(&self, connection: ConnectionId, request: &HookRequest) -> HookAnswer {
        let channel = lock(&self.inner.links).get(&connection).and_then(|link| link.channel.clone());
        channel.map_or(HookAnswer::Refused, |channel| channel.hook(request))
    }
}
