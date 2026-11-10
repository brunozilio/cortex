//! Delivers server events to the client and wakes it.

use crossbeam_channel::Sender;
use superfik_protocol::{ServerEvent, Wake};

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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
}
