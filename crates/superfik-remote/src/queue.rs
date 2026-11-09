//! What a connection that comes and goes keeps between sockets: the
//! commands waiting to go out, the greeting to send again first after a
//! reconnect, whether the app was told it was lost, and how long to wait
//! before the next attempt. Pure, so the browser's transport is tested
//! without a browser.

use std::collections::VecDeque;

pub const BACKOFF_MS: [u32; 6] = [500, 1_000, 2_000, 5_000, 10_000, 30_000];

#[derive(Debug, Default)]
pub struct Outgoing {
    pending: VecDeque<String>,
    hello: Option<String>,
    lost: bool,
    attempt: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Opened {
    pub restored: bool,
}

impl Outgoing {
    pub fn queue(&mut self, json: String, hello: bool) {
        if hello {
            self.hello = Some(json.clone());
        }
        self.pending.push_back(json);
    }

    pub fn opened(&mut self) -> Opened {
        self.attempt = 0;
        let restored = std::mem::take(&mut self.lost);
        if restored && let Some(hello) = self.hello.clone() {
            self.pending.push_front(hello);
        }
        Opened { restored }
    }

    pub fn closed(&mut self) -> bool {
        !std::mem::replace(&mut self.lost, true)
    }

    pub fn next_wait(&mut self) -> u32 {
        let wait = BACKOFF_MS[self.attempt.min(BACKOFF_MS.len() - 1)];
        self.attempt += 1;
        wait
    }

    pub fn take(&mut self) -> Option<String> {
        self.pending.pop_front()
    }

    pub fn put_back(&mut self, json: String) {
        self.pending.push_front(json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(out: &mut Outgoing) -> Vec<String> {
        std::iter::from_fn(|| out.take()).collect()
    }

    #[test]
    fn after_a_drop_the_greeting_goes_first_then_what_waited() {
        let mut out = Outgoing::default();
        out.queue("hello".into(), true);
        assert_eq!(out.opened(), Opened { restored: false });
        assert_eq!(drain(&mut out), ["hello"]);
        assert!(out.closed(), "told once");
        assert!(!out.closed(), "not twice");
        out.queue("prompt".into(), false);
        assert_eq!(out.opened(), Opened { restored: true });
        assert_eq!(drain(&mut out), ["hello", "prompt"]);
    }

    #[test]
    fn pauses_grow_then_stay_and_start_over_once_open() {
        let mut out = Outgoing::default();
        let waits: Vec<u32> = (0..8).map(|_| out.next_wait()).collect();
        assert_eq!(waits, [500, 1_000, 2_000, 5_000, 10_000, 30_000, 30_000, 30_000]);
        out.opened();
        assert_eq!(out.next_wait(), 500);
    }
}
