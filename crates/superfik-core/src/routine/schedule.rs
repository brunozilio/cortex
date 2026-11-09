//! When a routine fires. Pure arithmetic on unix seconds plus the UTC
//! offset the member scheduled it in, like `days`: no timezone database.

use serde::{Deserialize, Serialize};

use crate::days::{local_day, weekday};

const MINUTE: i64 = 60;
const DAY: i64 = 86_400;
const WEEK: i64 = 7 * DAY;
pub const MIN_INTERVAL_MINUTES: u32 = 5;
const MINUTES_PER_DAY: u16 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    Every { minutes: u32 },
    Daily { minute: u16, utc_offset: i32 },
    Weekly { weekday: u8, minute: u16, utc_offset: i32 },
}

impl Schedule {
    pub fn next_after(self, after: i64) -> i64 {
        match self {
            Self::Every { minutes } => after + i64::from(minutes.max(MIN_INTERVAL_MINUTES)) * MINUTE,
            Self::Daily { minute, utc_offset } => {
                let at = local_midnight(after, utc_offset) + minute_offset(minute);
                if at > after { at } else { at + DAY }
            }
            Self::Weekly { weekday: day, minute, utc_offset } => {
                let today = local_day(after, utc_offset);
                let ahead = (i64::from(day.min(6)) - weekday(today) as i64).rem_euclid(7);
                let at = local_midnight(after, utc_offset) + ahead * DAY + minute_offset(minute);
                if at > after { at } else { at + WEEK }
            }
        }
    }
}

fn local_midnight(unix: i64, utc_offset: i32) -> i64 {
    local_day(unix, utc_offset) * DAY - i64::from(utc_offset)
}

fn minute_offset(minute: u16) -> i64 {
    i64::from(minute.min(MINUTES_PER_DAY - 1)) * MINUTE
}
