use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use platform::sync::Locked;

use domain::schedule::{MOST, Moment, Scheduled, When};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde_json::Value;

use crate::model::ToolSpec;
use crate::tools::{Outcome, Tool, Workspace, field, required};

struct Entry {
    scheduled: Scheduled,
    made: i64,
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Entry>>> {
    static ALL: OnceLock<Mutex<HashMap<u64, Vec<Entry>>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).locked()
}

fn moment(secs: i64) -> Option<Moment> {
    moment_in(&TimeZone::system(), secs)
}

fn moment_in(zone: &TimeZone, secs: i64) -> Option<Moment> {
    let local = Timestamp::from_second(secs).ok()?.to_zoned(zone.clone());
    let field = |value: i8| u32::try_from(value).ok();
    Some(Moment {
        minute: field(local.minute())?,
        hour: field(local.hour())?,
        day: field(local.day())?,
        month: field(local.month())?,
        weekday: field(local.weekday().to_sunday_zero_offset())?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleError {
    #[error("{0}")]
    When(String),
    #[error("At most {MOST} prompts can be scheduled; delete some with CronDelete.")]
    Full,
}

impl From<ScheduleError> for String {
    fn from(error: ScheduleError) -> Self {
        error.to_string()
    }
}

pub fn add(client: u64, prompt: &str, when: When, now: i64) -> Result<Scheduled, ScheduleError> {
    let next = when.first(now, moment).map_err(ScheduleError::When)?;
    let mut all = lock();
    let mine = all.entry(client).or_default();
    if mine.len() >= MOST {
        return Err(ScheduleError::Full);
    }
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = format!("cron_{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let scheduled = Scheduled { id, prompt: prompt.to_owned(), when, next };
    mine.push(Entry { scheduled: scheduled.clone(), made: now });
    Ok(scheduled)
}

pub fn remove(client: u64, id: &str) -> bool {
    let mut all = lock();
    let Some(mine) = all.get_mut(&client) else { return false };
    let before = mine.len();
    mine.retain(|entry| entry.scheduled.id != id);
    mine.len() != before
}

pub fn list(client: u64) -> Vec<Scheduled> {
    let mut listed: Vec<Scheduled> =
        lock().get(&client).into_iter().flatten().map(|entry| entry.scheduled.clone()).collect();
    listed.sort_by_key(|scheduled| scheduled.next);
    listed
}

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

pub fn forget_client(client: u64) {
    lock().remove(&client);
}

pub fn now() -> i64 {
    Timestamp::now().as_second()
}

fn minute_in(zone: &TimeZone, secs: i64) -> Option<String> {
    let at = Timestamp::from_second(secs).ok()?.to_zoned(zone.clone());
    Some(at.strftime("%Y-%m-%d %H:%M").to_string())
}

fn shown(scheduled: &Scheduled) -> String {
    let at = minute_in(&TimeZone::system(), scheduled.next);
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

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(CronCreate), Box::new(CronList), Box::new(CronDelete), Box::new(ScheduleWakeup)]
}
