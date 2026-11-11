//! Prompts scheduled for later (`CronCreate`, `ScheduleWakeup`): a cron
//! expression in the user's local time, or a moment. They belong to the
//! client that made them and are sent to its session when it is idle.

use serde::{Deserialize, Serialize};

/// A scheduled prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheduled {
    pub id: String,
    pub prompt: String,
    pub when: When,
    /// When it next fires, in Unix seconds.
    pub next: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum When {
    /// A cron expression; once, or each time it matches until it expires.
    Cron { expr: String, recurring: bool },
    /// Once, at this moment (Unix seconds).
    At(i64),
}

/// A minute of the local calendar, as cron reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    pub minute: u32,
    pub hour: u32,
    pub day: u32,
    pub month: u32,
    /// 0 is Sunday.
    pub weekday: u32,
}

/// A five-field cron expression: minute, hour, day of month, month, day of
/// week. Each takes `*`, a number, `a-b`, a list `a,b` and a step `/n`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cron {
    minutes: u64,
    hours: u64,
    days: u64,
    months: u64,
    weekdays: u64,
    any_day: bool,
    any_weekday: bool,
}

impl Cron {
    pub fn parse(expr: &str) -> Result<Self, String> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = fields.as_slice() else {
            return Err(format!("A cron expression has five fields (minute hour day month weekday), not {expr:?}"));
        };
        let mut weekdays = field(weekday, 0, 7)?;
        if weekdays & (1 << 7) != 0 {
            weekdays |= 1; // 7 is Sunday too.
        }
        Ok(Self {
            minutes: field(minute, 0, 59)?,
            hours: field(hour, 0, 23)?,
            days: field(day, 1, 31)?,
            months: field(month, 1, 12)?,
            weekdays,
            any_day: *day == "*",
            any_weekday: *weekday == "*",
        })
    }

    /// Whether the expression fires at `at`. With both a day of month and
    /// a day of week given, either one is enough (as cron does).
    pub fn matches(&self, at: &Moment) -> bool {
        let has = |set: u64, n: u32| set & (1 << n) != 0;
        has(self.minutes, at.minute) && has(self.hours, at.hour) && has(self.months, at.month) && self.on_day(at)
    }
}

/// Prompts one client may have scheduled at once.
pub const MOST: usize = 50;
/// A recurring prompt ends a week after it was made.
pub const EXPIRES: i64 = 7 * 24 * 3600;
/// How far ahead a cron expression is looked for: a year.
const HORIZON: i64 = 366 * 24 * 3600;

impl Cron {
    /// The first minute after `now` (Unix seconds) it fires, reading the
    /// local calendar through `moment`. Whole hours and days that cannot
    /// match are skipped, so a year is a few hundred steps.
    pub fn next_after(&self, now: i64, moment: impl Fn(i64) -> Option<Moment>) -> Option<i64> {
        let has = |set: u64, n: u32| set & (1 << n) != 0;
        let mut at = now - now.rem_euclid(60) + 60;
        while at - now <= HORIZON {
            let m = moment(at)?;
            if !has(self.months, m.month) || !self.on_day(&m) {
                // To the next local midnight.
                at += i64::from((23 - m.hour.min(23)) * 60 + (60 - m.minute.min(59))) * 60;
            } else if !has(self.hours, m.hour) {
                at += i64::from(60 - m.minute.min(59)) * 60;
            } else if has(self.minutes, m.minute) {
                return Some(at);
            } else {
                at += 60;
            }
        }
        None
    }

    fn on_day(&self, at: &Moment) -> bool {
        let has = |set: u64, n: u32| set & (1 << n) != 0;
        match (self.any_day, self.any_weekday) {
            (true, true) => true,
            (true, false) => has(self.weekdays, at.weekday),
            (false, true) => has(self.days, at.day),
            (false, false) => has(self.days, at.day) || has(self.weekdays, at.weekday),
        }
    }
}

impl When {
    /// When it first fires after `now`.
    pub fn first(&self, now: i64, moment: impl Fn(i64) -> Option<Moment>) -> Result<i64, String> {
        match self {
            Self::Cron { expr, .. } => {
                Cron::parse(expr)?.next_after(now, moment).ok_or_else(|| format!("{expr} never fires within a year"))
            }
            Self::At(at) => Ok(*at),
        }
    }

    /// When a prompt made at `made` fires again after firing at `now`;
    /// `None` once it is done (a one-shot, or a recurring one expired).
    pub fn again(&self, made: i64, now: i64, moment: impl Fn(i64) -> Option<Moment>) -> Option<i64> {
        let Self::Cron { expr, recurring: true } = self else { return None };
        Cron::parse(expr).ok()?.next_after(now, moment).filter(|next| next - made < EXPIRES)
    }
}

fn field(text: &str, low: u32, high: u32) -> Result<u64, String> {
    let invalid = || format!("{text:?} is not a cron field within {low}-{high}");
    let mut set = 0_u64;
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => (range, step.parse::<u32>().ok().filter(|step| *step > 0).ok_or_else(invalid)?),
            None => (part, 1),
        };
        let (from, to) = match range {
            "*" => (low, high),
            _ => match range.split_once('-') {
                Some((from, to)) => (from.parse().map_err(|_| invalid())?, to.parse().map_err(|_| invalid())?),
                None => {
                    let at: u32 = range.parse().map_err(|_| invalid())?;
                    (at, if step > 1 { high } else { at })
                }
            },
        };
        if from < low || to > high || from > to {
            return Err(invalid());
        }
        for n in (from..=to).step_by(step as usize) {
            set |= 1 << n;
        }
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minute: u32, hour: u32, day: u32, month: u32, weekday: u32) -> Moment {
        Moment { minute, hour, day, month, weekday }
    }

    #[test]
    fn fields_take_numbers_ranges_lists_and_steps() {
        let every_five = Cron::parse("*/5 * * * *").expect("cron");
        assert!(every_five.matches(&at(35, 3, 9, 11, 1)));
        assert!(!every_five.matches(&at(36, 3, 9, 11, 1)));
        let workdays = Cron::parse("0 9 * * 1-5").expect("cron");
        assert!(workdays.matches(&at(0, 9, 10, 11, 2)));
        assert!(!workdays.matches(&at(0, 9, 15, 11, 6)), "Saturday");
        let listed = Cron::parse("15,45 8-10 1 1,7 *").expect("cron");
        assert!(listed.matches(&at(45, 10, 1, 7, 3)));
        assert!(!listed.matches(&at(45, 11, 1, 7, 3)));
    }

    #[test]
    fn a_day_of_month_or_of_week_is_enough_when_both_are_given() {
        let cron = Cron::parse("0 0 13 * 5").expect("cron");
        assert!(cron.matches(&at(0, 0, 13, 3, 2)), "the 13th");
        assert!(cron.matches(&at(0, 0, 7, 3, 5)), "a Friday");
        assert!(!cron.matches(&at(0, 0, 7, 3, 4)));
        assert!(Cron::parse("0 0 * * 7").expect("cron").matches(&at(0, 0, 1, 1, 0)), "7 is Sunday");
    }

    #[test]
    fn odd_expressions_are_refused() {
        for odd in ["* * * *", "60 * * * *", "*/0 * * * *", "5-1 * * * *", "a * * * *", "* * 0 * *"] {
            assert!(Cron::parse(odd).is_err(), "{odd}");
        }
    }
}

#[cfg(test)]
mod next_tests {
    use super::*;

    /// The calendar in UTC, for the tests.
    fn utc(secs: i64) -> Option<Moment> {
        let days = secs.div_euclid(86_400);
        let of_day = secs.rem_euclid(86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        Some(Moment {
            minute: u32::try_from(of_day / 60 % 60).ok()?,
            hour: u32::try_from(of_day / 3600).ok()?,
            day: u32::try_from(day).ok()?,
            month: u32::try_from(month).ok()?,
            weekday: u32::try_from((days + 4).rem_euclid(7)).ok()?,
        })
    }

    /// Tuesday 2026-11-10, 00:00 UTC.
    const TUESDAY: i64 = 20_767 * 86_400;

    #[test]
    fn the_next_time_skips_hours_and_days_that_cannot_match() {
        let next = |expr: &str, now: i64| Cron::parse(expr).expect("cron").next_after(now, utc);
        assert_eq!(next("0 9 * * 1-5", TUESDAY), Some(TUESDAY + 9 * 3600));
        assert_eq!(next("30 * * * *", TUESDAY + 600), Some(TUESDAY + 1800));
        assert_eq!(next("0 0 * * 6", TUESDAY), Some(TUESDAY + 4 * 86_400), "Saturday");
        assert_eq!(next("0 0 1 1 *", TUESDAY), Some(TUESDAY + 52 * 86_400), "New Year 2027");
        assert_eq!(next("0 0 29 2 *", TUESDAY), None, "no 29 February within a year");
    }

    #[test]
    fn a_recurring_prompt_fires_again_until_a_week_has_passed() {
        let hourly = When::Cron { expr: "0 * * * *".into(), recurring: true };
        assert_eq!(hourly.first(TUESDAY + 1, utc), Ok(TUESDAY + 3600));
        assert_eq!(hourly.again(TUESDAY, TUESDAY + 3600, utc), Some(TUESDAY + 7200));
        assert_eq!(hourly.again(TUESDAY, TUESDAY + EXPIRES, utc), None, "expired");
        let once = When::Cron { expr: "0 * * * *".into(), recurring: false };
        assert_eq!(once.again(TUESDAY, TUESDAY + 3600, utc), None);
        assert_eq!(When::At(5).again(0, 5, utc), None);
    }
}
