//! Prompts the agent schedules for later: `CronCreate` (a cron expression,
//! once or recurring), `ScheduleWakeup` (once, after a delay), listed with
//! `CronList` and removed with `CronDelete`. They belong to the client whose
//! run made them, outlive the run, and end when the client leaves
//! ([`forget_client`]); the client takes the due ones ([`take_due`]) when
//! its session is idle and sends them as prompts.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use domain::schedule::{MOST, Moment, Scheduled, When};
use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, field, required};
use crate::model::ToolSpec;

struct Entry {
    scheduled: Scheduled,
    made: i64,
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Entry>>> {
    static ALL: OnceLock<Mutex<HashMap<u64, Vec<Entry>>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner)
}

/// `secs` as a minute of the local calendar.
fn moment(secs: i64) -> Option<Moment> {
    let local: DateTime<Local> = Local.timestamp_opt(secs, 0).single()?;
    Some(Moment {
        minute: local.minute(),
        hour: local.hour(),
        day: local.day(),
        month: local.month(),
        weekday: local.weekday().num_days_from_sunday(),
    })
}

/// Schedules `prompt` for `client`.
pub fn add(client: u64, prompt: &str, when: When, now: i64) -> Result<Scheduled, String> {
    // Found before taking the lock, which every client shares.
    let next = when.first(now, moment)?;
    let mut all = lock();
    let mine = all.entry(client).or_default();
    if mine.len() >= MOST {
        return Err(format!("At most {MOST} prompts can be scheduled; delete some with CronDelete."));
    }
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = format!("cron_{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let scheduled = Scheduled { id, prompt: prompt.to_owned(), when, next };
    mine.push(Entry { scheduled: scheduled.clone(), made: now });
    Ok(scheduled)
}

/// Removes `client`'s scheduled prompt `id`; whether there was one.
pub fn remove(client: u64, id: &str) -> bool {
    let mut all = lock();
    let Some(mine) = all.get_mut(&client) else { return false };
    let before = mine.len();
    mine.retain(|entry| entry.scheduled.id != id);
    mine.len() != before
}

/// `client`'s scheduled prompts, the soonest first.
pub fn list(client: u64) -> Vec<Scheduled> {
    let mut listed: Vec<Scheduled> =
        lock().get(&client).into_iter().flatten().map(|entry| entry.scheduled.clone()).collect();
    listed.sort_by_key(|scheduled| scheduled.next);
    listed
}

/// The prompts due for `client` at `now`: one-shots leave, recurring ones
/// move to their next time (or leave once expired). A recurring prompt
/// missed several times fires once.
pub fn take_due(client: u64, now: i64) -> Vec<Scheduled> {
    let mut all = lock();
    let Some(mine) = all.get_mut(&client) else { return Vec::new() };
    let mut due = Vec::new();
    mine.retain_mut(|entry| {
        if entry.scheduled.next > now {
            return true;
        }
        due.push(entry.scheduled.clone());
        match entry.scheduled.when.again(entry.made, now, moment) {
            Some(next) => {
                entry.scheduled.next = next;
                true
            }
            None => false,
        }
    });
    due
}

/// Drops every prompt `client` scheduled.
pub fn forget_client(client: u64) {
    lock().remove(&client);
}

/// The time now, in Unix seconds.
pub fn now() -> i64 {
    Local::now().timestamp()
}

fn shown(scheduled: &Scheduled) -> String {
    let at = Local.timestamp_opt(scheduled.next, 0).single().map(|at| at.format("%Y-%m-%d %H:%M").to_string());
    let how = match &scheduled.when {
        When::Cron { expr, recurring: true } => format!("`{expr}`, recurring"),
        When::Cron { expr, recurring: false } => format!("`{expr}`, once"),
        When::At(_) => "once".to_owned(),
    };
    format!("{} ({how}), next {}: {}", scheduled.id, at.unwrap_or_default(), scheduled.prompt)
}

#[cfg(test)]
mod tests;

mod tools;
pub use tools::{CronCreate, CronDelete, CronList, ScheduleWakeup};
