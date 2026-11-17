use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::api::{Reply, RequestId, ServerEvent, Unsolicited, Wake};
use crossbeam_channel::Sender;

#[derive(Clone)]
pub struct Outbox {
    sender: Sender<ServerEvent>,
    wake: Wake,
}

impl std::fmt::Debug for Outbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outbox").finish_non_exhaustive()
    }
}

impl Outbox {
    pub fn new(sender: Sender<ServerEvent>, wake: Wake) -> Self {
        Self { sender, wake }
    }

    pub fn send(&self, event: ServerEvent) {
        if self.sender.send(event).is_ok() {
            (self.wake)();
        } else {
            tracing::debug!("client gone; dropping event");
        }
    }

    pub fn wake(&self) -> Wake {
        Arc::clone(&self.wake)
    }

    pub fn answering(&self, id: RequestId) -> Replies {
        Replies { outbox: self.clone(), id, stop: Arc::default() }
    }
}

#[derive(Debug, Clone)]
pub struct Replies {
    outbox: Outbox,
    id: RequestId,
    stop: Arc<AtomicBool>,
}

impl Replies {
    pub fn id(&self) -> RequestId {
        self.id
    }

    pub fn send(&self, reply: Reply) {
        self.outbox.send(ServerEvent::Reply { id: self.id, reply });
    }

    pub fn tell(&self, unsolicited: Unsolicited) {
        self.outbox.send(ServerEvent::Unsolicited(unsolicited));
    }

    pub fn stop(&self) -> &Arc<AtomicBool> {
        &self.stop
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn done(id: u64) -> ServerEvent {
        ServerEvent::Reply { id: RequestId(id), reply: Reply::Done }
    }

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
        outbox.send(done(1));
        assert_eq!(receiver.try_recv().ok(), Some(done(1)));
        drop(receiver);
        outbox.send(done(2));
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn every_reply_names_the_request_it_answers() {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let outbox = Outbox::new(sender, Arc::new(|| {}));
        outbox.answering(RequestId(7)).send(Reply::error("no"));
        outbox.answering(RequestId(8)).send(Reply::Done);
        assert_eq!(receiver.try_recv().ok(), Some(ServerEvent::Reply { id: RequestId(7), reply: Reply::error("no") }));
        assert_eq!(receiver.try_recv().ok(), Some(done(8)));
    }
}
