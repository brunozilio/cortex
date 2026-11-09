//! What goes out of a channel account: replies kept as drafts for a
//! member, and sends handed to the account's paced sender.

use superfik_core::channel::limits::Priority;
use superfik_core::channel::{Attachment, Source};
use superfik_core::{ConnectionId, Principal, RunId};
use superfik_protocol::ServerEvent;

use super::sender::Outgoing;
use super::{Channels, lock};

impl Channels {
    pub fn draft(
        &self,
        owner: &Principal,
        connection: ConnectionId,
        exchange: &str,
        text: &str,
        attachment: Option<Attachment>,
        run: RunId,
    ) -> ServerEvent {
        let now = self.inner.clock.now_ms();
        self.change(owner, connection, |book| {
            book.draft(exchange, text, attachment, run, now);
            true
        });
        self.update(owner, connection)
    }

    pub fn send(
        &self,
        owner: &Principal,
        connection: ConnectionId,
        exchange: &str,
        text: &str,
        attachment: Option<Attachment>,
    ) -> Result<(), String> {
        let book = self.book(owner, connection);
        let last_contact = book
            .thread(exchange)
            .and_then(|t| t.messages.iter().filter(|m| m.source == Source::Contact).map(|m| m.at).max());
        let priority = if last_contact.is_some() { Priority::Reply } else { Priority::New };
        let links = lock(&self.inner.links);
        let link = links.get(&connection).filter(|l| &l.owner == owner).ok_or("this account is not linked")?;
        if !link.kind.may_answer(last_contact, self.inner.clock.now_ms()) {
            return Err(format!(
                "{} lets a business answer only within 24 hours of the person's last message; \
                 this conversation is outside that window (WhatsApp needs an approved template there)",
                link.kind.name()
            ));
        }
        let queue = link.queue.as_ref().ok_or("the account is not connected yet")?;
        let outgoing = Outgoing { exchange: exchange.to_owned(), text: text.to_owned(), priority, attachment };
        queue.send(outgoing).map_err(|_| "the account's sender stopped".to_owned())
    }

    pub fn settle_draft(
        &self,
        owner: &Principal,
        connection: ConnectionId,
        id: u32,
        send: Option<&str>,
    ) -> Result<ServerEvent, String> {
        let mut taken = None;
        self.change(owner, connection, |book| {
            taken = book.take_draft(id);
            taken.is_some()
        });
        let draft = taken.ok_or("that draft is gone")?;
        if let Some(text) = send {
            let text = if text.trim().is_empty() { draft.text.as_str() } else { text };
            self.send(owner, connection, &draft.exchange, text, draft.attachment.clone())?;
        }
        Ok(self.update(owner, connection))
    }
}
