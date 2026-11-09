//! Delivers server events to the client and wakes it.
//!
//! Every event has a [`Recipient`]: a run's events go to the run, the rest
//! to whoever this outbox answers (`to`; everyone by default). A local
//! backend has one client, so its outbox drops the address; a server's
//! keeps it to fan events out.

use crossbeam_channel::Sender;
use superfik_core::Principal;
use superfik_protocol::{Addressed, Recipient, ServerEvent};

use superfik_protocol::Wake;

#[derive(Clone)]
enum Channel {
    Local(Sender<ServerEvent>),
    Addressed(Sender<Addressed>),
}

#[derive(Clone)]
pub struct Outbox {
    channel: Channel,
    to: Recipient,
    wake: Wake,
}

impl std::fmt::Debug for Outbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outbox").field("to", &self.to).finish_non_exhaustive()
    }
}

impl Outbox {
    pub fn new(sender: Sender<ServerEvent>, wake: Wake) -> Self {
        Self { channel: Channel::Local(sender), to: Recipient::Everyone, wake }
    }

    pub fn addressed(sender: Sender<Addressed>, wake: Wake) -> Self {
        Self { channel: Channel::Addressed(sender), to: Recipient::Everyone, wake }
    }

    pub fn answering(&self, principal: &Principal) -> Self {
        Self { to: Recipient::Principal(principal.clone()), ..self.clone() }
    }

    pub fn send(&self, event: ServerEvent) {
        let delivered = match &self.channel {
            Channel::Local(sender) => sender.send(event).is_ok(),
            Channel::Addressed(sender) => sender.send(Addressed { to: self.recipient(&event), event }).is_ok(),
        };
        if delivered {
            (self.wake)();
        } else {
            tracing::debug!("client gone; dropping event");
        }
    }

    fn recipient(&self, event: &ServerEvent) -> Recipient {
        match event {
            ServerEvent::Run { run, .. } => Recipient::Run(*run),
            _ => self.to.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use superfik_core::{EngineEvent, OrgId, RunId, UserId};

    use super::*;

    #[test]
    fn sends_and_wakes_only_when_delivered() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&wakes);
        let (sender, receiver) = crossbeam_channel::unbounded();
        let outbox = Outbox::new(
            sender,
            Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );
        outbox.send(ServerEvent::error("a"));
        assert_eq!(receiver.try_recv().ok(), Some(ServerEvent::error("a")));
        drop(receiver);
        outbox.send(ServerEvent::error("b"));
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn answers_go_to_the_sender_and_run_events_to_the_run() {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let member = Principal::member(UserId::new(), OrgId::new());
        let outbox = Outbox::addressed(sender, Arc::new(|| {})).answering(&member);
        let run = RunId::new();
        outbox.send(ServerEvent::error("no"));
        outbox.send(ServerEvent::Run { run, event: EngineEvent::Cancelled });
        let sent: Vec<Recipient> = receiver.try_iter().map(|addressed| addressed.to).collect();
        assert_eq!(sent, [Recipient::Principal(member), Recipient::Run(run)]);
    }

    #[test]
    fn an_outbox_answering_nobody_speaks_to_everyone() {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Outbox::addressed(sender, Arc::new(|| {})).send(ServerEvent::error("restarting"));
        assert_eq!(receiver.try_recv().map(|addressed| addressed.to).ok(), Some(Recipient::Everyone));
    }
}
