//! Grouping moments by local calendar day ("Today", "Yesterday", a date).
//! Pure integer arithmetic on unix seconds plus a UTC offset, so it needs
//! no timezone database; the shell supplies the offset.

use std::fmt;

const DAY: i64 = 86_400;
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DayGroup {
    Today,
    Yesterday,
    Day(i64),
}

pub fn local_day(unix: i64, utc_offset_secs: i32) -> i64 {
    (unix + i64::from(utc_offset_secs)).div_euclid(DAY)
}

pub fn clock_time(unix: i64, utc_offset_secs: i32) -> String {
    let secs = (unix + i64::from(utc_offset_secs)).rem_euclid(DAY);
    format!("{:02}:{:02}", secs / 3600, secs % 3600 / 60)
}

pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

pub fn weekday(days: i64) -> usize {
    (days + 3).rem_euclid(7) as usize
}

impl DayGroup {
    pub fn of(unix: i64, now_unix: i64, utc_offset_secs: i32) -> Self {
        let day = local_day(unix, utc_offset_secs);
        match local_day(now_unix, utc_offset_secs) - day {
            0 => Self::Today,
            1 => Self::Yesterday,
            _ => Self::Day(day),
        }
    }

    pub fn label(self, now_unix: i64, utc_offset_secs: i32) -> String {
        let Self::Day(day) = self else { return self.to_string() };
        let today = local_day(now_unix, utc_offset_secs);
        let age = today - day;
        if (2..7).contains(&age) {
            return WEEKDAYS[weekday(day)].to_owned();
        }
        let (year, month, date) = civil(day);
        let month = MONTHS[(month - 1) as usize];
        if year == civil(today).0 { format!("{month} {date}") } else { format!("{month} {date}, {year}") }
    }
}

impl fmt::Display for DayGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Today => f.write_str("Today"),
            Self::Yesterday => f.write_str("Yesterday"),
            Self::Day(day) => write!(f, "day {day}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_793_282_400;

    #[test]
    fn civil_dates_round_trip_known_days() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(local_day(NOW, 0)), (2026, 10, 29));
        assert_eq!(weekday(0), 3, "1970-01-01 was a Thursday");
        assert_eq!(WEEKDAYS[weekday(local_day(NOW, 0))], "Thursday");
    }

    #[test]
    fn groups_relative_to_now() {
        assert_eq!(DayGroup::of(NOW - 3600, NOW, 0), DayGroup::Today);
        assert_eq!(DayGroup::of(NOW - DAY, NOW, 0), DayGroup::Yesterday);
        assert_eq!(DayGroup::of(NOW - 3 * DAY, NOW, 0).label(NOW, 0), "Monday");
        assert_eq!(DayGroup::of(NOW - 9 * DAY, NOW, 0).label(NOW, 0), "Oct 20");
        assert_eq!(DayGroup::of(NOW - 400 * DAY, NOW, 0).label(NOW, 0), "Sep 24, 2025");
        assert_eq!(DayGroup::Today.label(NOW, 0), "Today");
        assert_eq!(DayGroup::Yesterday.label(NOW, 0), "Yesterday");
    }

    #[test]
    fn clock_time_is_local_hours_and_minutes() {
        assert_eq!(clock_time(0, 0), "00:00");
        assert_eq!(clock_time(3 * 3600 + 5 * 60, 0), "03:05");
        assert_eq!(clock_time(3600, -2 * 3600), "23:00", "wraps across midnight");
        assert_eq!(clock_time(1_761_900_000, -3 * 3600), "05:40", "08:40 UTC at UTC-3");
    }

    #[test]
    fn midnight_depends_on_the_local_offset() {
        let late = NOW - 14 * 3600 - 1800;
        assert_eq!(DayGroup::of(late, NOW, 0), DayGroup::Yesterday);
        assert_eq!(DayGroup::of(late, NOW, 3600), DayGroup::Today, "00:30 local on the 29th");
        assert_eq!(DayGroup::of(late, NOW, -3 * 3600), DayGroup::Yesterday);
    }

    #[test]
    fn groups_order_newest_first() {
        assert!(DayGroup::Today < DayGroup::Yesterday);
        assert!(DayGroup::Yesterday < DayGroup::Day(5));
    }
}
