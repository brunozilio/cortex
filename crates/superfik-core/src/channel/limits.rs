//! The ceilings of a number's sends. A send under a ceiling never fails: it
//! waits for its window and goes out on its own.
//!
//! - Messages in a sliding hour, messages a day, and new conversations a
//!   day (a contact who never wrote to the number), counted once per
//!   contact a day. The day is the member's local day, from a UTC offset
//!   like the rest of Superfik (no timezone database: a daylight-saving
//!   change moves the day's edge by an hour).
//! - `Planner::allowed_at` is the rule; `plan_expected_at` places a queue
//!   ahead of a send, one send's pace apart, to tell when it should go out.
//!
//! Times are unix milliseconds.

use serde::{Deserialize, Serialize};

const HOUR_MS: i64 = 60 * 60 * 1_000;
const DAY_MS: i64 = 24 * HOUR_MS;
pub const ESTIMATED_SEND_MS: i64 = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub per_hour: usize,
    pub per_day: usize,
    pub new_per_day: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { per_hour: 150, per_day: 800, new_per_day: 50 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Reply,
    New,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSend {
    pub priority: Priority,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    pub sent: Vec<i64>,
    pub opened: Vec<(i64, String)>,
}

#[derive(Debug, Clone)]
pub struct Planner {
    limits: Limits,
    utc_offset_ms: i64,
    sent: Vec<i64>,
    opened: Vec<(i64, String)>,
}

impl Planner {
    pub fn new(limits: Limits, ledger: Ledger, utc_offset_secs: i32) -> Self {
        let utc_offset_ms = i64::from(utc_offset_secs) * 1_000;
        Self { limits, utc_offset_ms, sent: ledger.sent, opened: ledger.opened }
    }

    fn day(&self, at: i64) -> (i64, i64) {
        let local = at + self.utc_offset_ms;
        let start = local - local.rem_euclid(DAY_MS) - self.utc_offset_ms;
        (start, start + DAY_MS)
    }

    pub fn allowed_at(&self, send: &PlannedSend, now: i64) -> i64 {
        let mut at = now;
        let per_hour = self.limits.per_hour.max(1);
        if self.sent.len() >= per_hour {
            at = at.max(self.sent[self.sent.len() - per_hour] + HOUR_MS);
        }
        for _ in 0..3 {
            let (start, end) = self.day(at);
            let today = self.sent.len() - self.sent.partition_point(|sent| *sent < start);
            let opened_today = self.opened.len() - self.opened.partition_point(|(opened, _)| *opened < start);
            let known = send
                .to
                .as_ref()
                .is_some_and(|to| self.opened.iter().any(|(opened, who)| who == to && *opened >= start));
            let new_full = send.priority == Priority::New && !known && opened_today >= self.limits.new_per_day;
            if today < self.limits.per_day && !new_full {
                break;
            }
            at = end;
        }
        at
    }

    pub fn record(&mut self, send: &PlannedSend, at: i64) {
        self.sent.push(at);
        if send.priority == Priority::New
            && let Some(to) = &send.to
            && !self.opened.iter().any(|(_, who)| who == to)
        {
            self.opened.push((at, to.clone()));
        }
    }
}

pub fn plan_expected_at(
    limits: Limits,
    ledger: Ledger,
    ahead: &[PlannedSend],
    send: &PlannedSend,
    now: i64,
    utc_offset_secs: i32,
) -> i64 {
    let mut planner = Planner::new(limits, ledger, utc_offset_secs);
    let mut floor = now;
    for queued in ahead {
        let at = planner.allowed_at(queued, floor);
        planner.record(queued, at);
        floor = now.max(at + ESTIMATED_SEND_MS);
    }
    planner.allowed_at(send, floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: i64 = 1_793_016_000_000;
    const BRT: i32 = -3 * 3_600;

    fn reply() -> PlannedSend {
        PlannedSend { priority: Priority::Reply, to: Some("5511999990000".into()) }
    }

    fn new_to(to: &str) -> PlannedSend {
        PlannedSend { priority: Priority::New, to: Some(to.into()) }
    }

    fn limits(per_hour: usize, per_day: usize, new_per_day: usize) -> Limits {
        Limits { per_hour, per_day, new_per_day }
    }

    #[test]
    fn under_every_ceiling_a_send_goes_now() {
        let planner = Planner::new(Limits::default(), Ledger::default(), BRT);
        assert_eq!(planner.allowed_at(&reply(), NOON), NOON);
    }

    #[test]
    fn a_full_hour_waits_for_its_oldest_send_to_leave() {
        let sent = vec![NOON - 50 * 60_000, NOON - 40 * 60_000, NOON - 10 * 60_000];
        let planner = Planner::new(limits(3, 100, 10), Ledger { sent, opened: vec![] }, BRT);
        assert_eq!(planner.allowed_at(&reply(), NOON), NOON + 10 * 60_000);
    }

    #[test]
    fn a_full_day_waits_for_the_next_local_midnight() {
        let sent = vec![NOON - 2 * HOUR_MS, NOON - HOUR_MS];
        let planner = Planner::new(limits(100, 2, 10), Ledger { sent, opened: vec![] }, BRT);
        assert_eq!(planner.allowed_at(&reply(), NOON), NOON + 15 * HOUR_MS);
    }

    #[test]
    fn new_conversations_wait_at_their_ceiling_but_replies_and_known_contacts_do_not() {
        let opened = vec![(NOON - HOUR_MS, "a".to_owned())];
        let ledger = Ledger { sent: vec![NOON - HOUR_MS], opened };
        let planner = Planner::new(limits(100, 100, 1), ledger, BRT);
        assert_eq!(planner.allowed_at(&reply(), NOON), NOON);
        assert_eq!(planner.allowed_at(&new_to("a"), NOON), NOON, "already opened today");
        assert_eq!(planner.allowed_at(&new_to("b"), NOON), NOON + 15 * HOUR_MS);
    }

    #[test]
    fn a_queue_is_placed_send_after_send_across_hours() {
        let ahead = vec![reply(), reply()];
        let at = plan_expected_at(limits(2, 100, 10), Ledger::default(), &ahead, &reply(), NOON, BRT);
        assert_eq!(at, NOON + HOUR_MS);
        let none = plan_expected_at(Limits::default(), Ledger::default(), &[], &reply(), NOON, BRT);
        assert_eq!(none, NOON);
        let one = plan_expected_at(Limits::default(), Ledger::default(), &[reply()], &reply(), NOON, BRT);
        assert_eq!(one, NOON + ESTIMATED_SEND_MS);
    }
}
