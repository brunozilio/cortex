use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use domain::schedule::{MOST, Moment, Scheduled, When};
use serde_json::{Value, json};

use crate::tools::{Access, Outcome, Tool, Workspace, field, required};
use crate::model::ToolSpec;

struct Entry {
    scheduled: Scheduled,
    made: i64,
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Entry>>> {
    static ALL: OnceLock<Mutex<HashMap<u64, Vec<Entry>>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner)
}

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

pub fn add(client: u64, prompt: &str, when: When, now: i64) -> Result<Scheduled, String> {
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

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(CronCreate), Box::new(CronList), Box::new(CronDelete), Box::new(ScheduleWakeup)]
}
