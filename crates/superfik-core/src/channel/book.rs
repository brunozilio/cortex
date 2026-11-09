//! One account's conversations: what each one said lately, whether the
//! agent may answer it, and the replies waiting for a member's approval.
//! What a store keeps per connection; pure.

use serde::{Deserialize, Serialize};

use super::limits::Ledger;
use super::{Exchange, Message, Verdict};
use crate::RunId;

pub const KEPT_MESSAGES: usize = 200;
pub const KEPT_EXCHANGES: usize = 500;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    pub exchange: Exchange,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub id: u32,
    pub exchange: String,
    pub text: String,
    pub run: RunId,
    pub written: i64,
    #[serde(default)]
    pub attachment: Option<super::Attachment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelBook {
    #[serde(default)]
    pub threads: Vec<Thread>,
    #[serde(default)]
    pub drafts: Vec<Draft>,
    #[serde(default)]
    pub next_draft: u32,
}

impl ChannelBook {
    pub fn thread(&self, exchange: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.exchange.id == exchange)
    }

    pub fn receive(&mut self, message: Message, pause_ms: i64) -> Option<Verdict> {
        let at = match self.threads.iter().position(|t| t.exchange.id == message.exchange) {
            Some(at) => at,
            None => {
                self.threads
                    .insert(0, Thread { exchange: Exchange::new(message.exchange.clone()), ..Thread::default() });
                0
            }
        };
        let mut thread = self.threads.remove(at);
        if thread.messages.iter().any(|kept| kept.id == message.id) {
            self.threads.insert(at, thread);
            return None;
        }
        let verdict = thread.exchange.observe(&message, pause_ms);
        if message.source == super::Source::Contact && !message.is_group && message.sender_name.is_some() {
            thread.name.clone_from(&message.sender_name);
        }
        let position = thread.messages.partition_point(|kept| kept.at <= message.at);
        thread.messages.insert(position, message);
        if thread.messages.len() > KEPT_MESSAGES {
            let extra = thread.messages.len() - KEPT_MESSAGES;
            thread.messages.drain(..extra);
        }
        self.threads.insert(0, thread);
        self.threads.truncate(KEPT_EXCHANGES);
        Some(verdict)
    }

    pub fn draft(
        &mut self,
        exchange: &str,
        text: &str,
        attachment: Option<super::Attachment>,
        run: RunId,
        now: i64,
    ) -> u32 {
        let id = self.next_draft;
        self.next_draft += 1;
        let draft = Draft { id, exchange: exchange.to_owned(), text: text.to_owned(), run, written: now, attachment };
        self.drafts.push(draft);
        id
    }

    pub fn take_draft(&mut self, id: u32) -> Option<Draft> {
        let at = self.drafts.iter().position(|d| d.id == id)?;
        Some(self.drafts.remove(at))
    }

    pub fn ledger(&self, since: i64) -> Ledger {
        let mut sent = Vec::new();
        let mut opened = Vec::new();
        for thread in &self.threads {
            let mut contact_wrote = false;
            for message in &thread.messages {
                match message.source {
                    super::Source::Contact => contact_wrote = true,
                    super::Source::Agent if message.at >= since => {
                        sent.push(message.at);
                        if !contact_wrote && !opened.iter().any(|(_, to): &(i64, String)| to == &thread.exchange.id) {
                            opened.push((message.at, thread.exchange.id.clone()));
                        }
                    }
                    _ => {}
                }
            }
        }
        sent.sort_unstable();
        opened.sort_by_key(|(at, _)| *at);
        Ledger { sent, opened }
    }

    pub fn recent(&self, exchange: &str, limit: usize) -> &[Message] {
        self.thread(exchange).map_or(&[], |t| &t.messages[t.messages.len().saturating_sub(limit)..])
    }
}

#[cfg(test)]
mod tests {
    use super::super::{PAUSE_MS, Source};
    use super::*;

    fn message(id: &str, exchange: &str, source: Source, at: i64) -> Message {
        Message {
            id: id.into(),
            exchange: exchange.into(),
            is_group: false,
            sender: Some(exchange.into()),
            sender_name: (source == Source::Contact).then(|| "Ana".to_owned()),
            text: format!("text {id}"),
            media: None,
            mentioned_me: false,
            reply_to_me: false,
            at,
            source,
        }
    }

    #[test]
    fn messages_join_their_thread_once_and_the_thread_moves_first() {
        let mut book = ChannelBook::default();
        assert_eq!(book.receive(message("1", "a", Source::Contact, 10), PAUSE_MS), Some(Verdict::Answer));
        assert_eq!(book.receive(message("2", "b", Source::Contact, 20), PAUSE_MS), Some(Verdict::Answer));
        assert_eq!(book.receive(message("1", "a", Source::Contact, 10), PAUSE_MS), None, "seen before");
        assert_eq!(book.threads[0].exchange.id, "b");
        book.receive(message("3", "a", Source::Agent, 30), PAUSE_MS);
        assert_eq!(book.threads[0].exchange.id, "a");
        assert_eq!(book.thread("a").map(|t| t.messages.len()), Some(2));
        assert_eq!(book.thread("a").and_then(|t| t.name.clone()), Some("Ana".into()));
    }

    #[test]
    fn late_messages_are_kept_in_time_order() {
        let mut book = ChannelBook::default();
        book.receive(message("2", "a", Source::Contact, 20), PAUSE_MS);
        book.receive(message("1", "a", Source::Contact, 10), PAUSE_MS);
        let ids: Vec<&str> = book.recent("a", 10).iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["1", "2"]);
        assert_eq!(book.recent("a", 1).len(), 1);
        assert!(book.recent("nobody", 5).is_empty());
    }

    #[test]
    fn a_thread_keeps_its_latest_messages() {
        let mut book = ChannelBook::default();
        for i in 0..(KEPT_MESSAGES as i64 + 3) {
            book.receive(message(&i.to_string(), "a", Source::Contact, i), PAUSE_MS);
        }
        let kept = book.recent("a", usize::MAX);
        assert_eq!(kept.len(), KEPT_MESSAGES);
        assert_eq!(kept[0].id, "3");
    }

    #[test]
    fn a_member_on_the_phone_pauses_the_thread() {
        let mut book = ChannelBook::default();
        book.receive(message("1", "a", Source::Member, 10), PAUSE_MS);
        assert_eq!(book.receive(message("2", "a", Source::Contact, 20), PAUSE_MS), Some(Verdict::Keep));
    }

    #[test]
    fn the_ledger_counts_the_agents_sends_and_the_conversations_it_opened() {
        let mut book = ChannelBook::default();
        book.receive(message("1", "ana", Source::Contact, 10), PAUSE_MS);
        book.receive(message("2", "ana", Source::Agent, 20), PAUSE_MS);
        book.receive(message("3", "bruno", Source::Agent, 30), PAUSE_MS);
        book.receive(message("4", "bruno", Source::Agent, 40), PAUSE_MS);
        book.receive(message("5", "carla", Source::Agent, 1), PAUSE_MS);
        book.receive(message("6", "ana", Source::Member, 50), PAUSE_MS);
        let ledger = book.ledger(5);
        assert_eq!(ledger.sent, vec![20, 30, 40]);
        assert_eq!(ledger.opened, vec![(30, "bruno".to_owned())]);
    }

    #[test]
    fn drafts_wait_until_taken() {
        let mut book = ChannelBook::default();
        let run = RunId::new();
        let first = book.draft("a", "Olá!", None, run, 5);
        let second = book.draft("b", "Oi", None, run, 6);
        assert_ne!(first, second);
        assert_eq!(book.take_draft(first).map(|d| d.text), Some("Olá!".into()));
        assert!(book.take_draft(first).is_none());
        assert_eq!(book.drafts.len(), 1);
    }
}
