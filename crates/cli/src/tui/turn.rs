use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use app::RunControl;

use super::rewind::Checkpoint;
use super::stopwatch::Stopwatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket(u64);

impl Ticket {
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Default)]
pub struct Turn {
    pub run: Option<RunControl>,
    pub started: Option<Stopwatch>,
    pub queued: Vec<String>,
    pub(super) unresolved: VecDeque<String>,
    pub(super) held: Vec<String>,
    preparing: Option<(Ticket, String)>,
    pub(super) retrying: bool,
    pub(super) transient: bool,
    pub(super) auto_retried: bool,
    pub auto_retry: Option<std::time::Instant>,
    pub(super) rewound: Option<domain::Rewound>,
    pub checkpoints: Vec<Checkpoint>,
}

impl Turn {
    pub fn busy(&self) -> bool {
        self.run.is_some() || self.preparing.is_some()
    }

    pub fn preparing(&self) -> bool {
        self.preparing.is_some()
    }

    pub fn prepare(&mut self, typed: &str) -> Ticket {
        let ticket = Ticket::next();
        self.preparing = Some((ticket, typed.to_owned()));
        ticket
    }

    pub fn waits_for(&self, ticket: Ticket) -> bool {
        self.preparing.as_ref().is_some_and(|(waited, _)| *waited == ticket)
    }

    pub fn take_back(&mut self) -> Option<String> {
        let (_, typed) = self.preparing.take()?;
        let typed_after = std::mem::take(&mut self.held);
        Some(std::iter::once(typed).chain(typed_after).collect::<Vec<_>>().join("\n\n"))
    }

    pub fn prepared(&mut self, ticket: Ticket) -> bool {
        let waited = self.waits_for(ticket);
        if waited {
            self.preparing = None;
        }
        waited
    }

    pub fn runs(&self, run: domain::RunId) -> bool {
        self.run.as_ref().is_some_and(|control| control.id() == run)
    }

    pub fn waiting(&self) -> impl Iterator<Item = &String> {
        self.queued.iter().chain(&self.unresolved).chain(&self.held)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_is_busy_only_while_a_run_goes_on() {
        let idle = Turn::default();
        assert!(!idle.busy());
        assert!(idle.queued.is_empty() && idle.checkpoints.is_empty() && !idle.retrying);
    }

    #[test]
    fn a_turn_is_busy_while_a_prompt_is_prepared_and_only_its_own_ticket_ends_that() {
        let mut turn = Turn::default();
        let first = turn.prepare("one");
        assert!(turn.busy() && turn.preparing());
        let second = turn.prepare("two");
        assert!(!turn.prepared(first), "an older ticket was replaced");
        assert!(turn.busy());
        assert!(turn.prepared(second));
        assert!(!turn.busy() && !turn.prepared(second), "a ticket is used once");
    }

    #[test]
    fn taking_back_a_prompt_being_prepared_gives_what_was_typed_and_its_ticket_no_longer_counts() {
        let mut turn = Turn::default();
        assert_eq!(turn.take_back(), None, "nothing is being prepared");
        let ticket = turn.prepare("read @notes.txt");
        turn.held.push("and then this".into());

        assert_eq!(turn.take_back().as_deref(), Some("read @notes.txt\n\nand then this"));

        assert!(!turn.busy() && turn.held.is_empty());
        assert!(!turn.waits_for(ticket) && !turn.prepared(ticket), "the worker's late result is nobody's");
    }

    #[test]
    fn what_waits_shows_in_the_order_it_will_reach_the_agent() {
        let mut turn = Turn::default();
        turn.held.push("typed while preparing".into());
        turn.unresolved.push_back("names a file".into());
        turn.queued.push("handed over".into());
        assert_eq!(turn.waiting().collect::<Vec<_>>(), ["handed over", "names a file", "typed while preparing"]);
    }
}
