//! What the Routines screen says about routines and reports, as text.
//! Pure, so the wording is tested apart from the painting.

use superfik_core::DayGroup;
use superfik_core::job::JobSignal;
use superfik_core::routine::{Autonomy, Ending, Report, Routine, Schedule};
use superfik_i18n::{Strings, Text};

use crate::state::routines::Cadence;

pub fn schedule(strings: &Strings, schedule: Schedule) -> String {
    match schedule {
        Schedule::Every { minutes } => strings.t(Text::RoutineEvery(minutes)),
        Schedule::Daily { minute, .. } => strings.t(Text::RoutineDaily(&clock(minute))),
        Schedule::Weekly { weekday, minute, .. } => {
            strings.t(Text::RoutineWeekly(usize::from(weekday), &clock(minute)))
        }
    }
}

pub fn clock(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

pub fn autonomy(autonomy: Autonomy) -> Text<'static> {
    match autonomy {
        Autonomy::Draft => Text::AutonomyDraft,
        Autonomy::Bounded => Text::AutonomyBounded,
        Autonomy::Auto => Text::AutonomyAuto,
    }
}

pub fn cadence(cadence: Cadence) -> Text<'static> {
    match cadence {
        Cadence::Hourly => Text::CadenceHourly,
        Cadence::Daily => Text::CadenceDaily,
        Cadence::Weekly => Text::CadenceWeekly,
    }
}

pub fn routine_detail(strings: &Strings, routine: &Routine) -> String {
    if !routine.enabled {
        return strings.t(Text::RoutinePaused);
    }
    format!("{} · {}", schedule(strings, routine.schedule), strings.t(autonomy(routine.autonomy)))
}

pub fn report_detail(strings: &Strings, report: &Report, now_unix: i64, utc_offset: i32) -> String {
    let local = report.started + i64::from(utc_offset);
    let time = clock(u16::try_from(local.rem_euclid(86_400) / 60).unwrap_or(0));
    let day = strings.day(DayGroup::of(report.started, now_unix, utc_offset), now_unix, utc_offset);
    let mut parts = vec![format!("{day} {time}")];
    match &report.ending {
        None => parts.push(strings.t(Text::ReportRunning)),
        Some(Ending::Completed) => {}
        Some(Ending::Failed(_)) => parts.push(strings.t(Text::ReportFailed)),
        Some(Ending::Cancelled) => parts.push(strings.t(Text::ReportStopped)),
    }
    let signal = report.signal().filter(|_| report.ending.is_some());
    let first = match (&signal, &report.ending) {
        (Some(JobSignal::Result(headline)), _) => headline.as_str(),
        (Some(JobSignal::NeedsInput(what)), _) => {
            parts.push(strings.t(Text::ReportNeedsInput));
            what.as_str()
        }
        (Some(JobSignal::Failed(why)), Some(Ending::Completed)) => {
            parts.push(strings.t(Text::ReportFailed));
            why.as_str()
        }
        (Some(JobSignal::Failed(why)), _) => why.as_str(),
        (None, Some(Ending::Failed(why))) if report.text.trim().is_empty() => why.as_str(),
        (None, _) => report.text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or(""),
    };
    if !first.is_empty() {
        parts.push(first.trim_start_matches('#').trim().to_owned());
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use superfik_core::RunId;
    use superfik_core::routine::RoutineId;
    use superfik_i18n::Language;

    use super::*;

    const MONDAY: i64 = 1_792_972_800;

    fn en() -> Strings {
        Strings::new(Language::English)
    }

    fn report(ending: Option<Ending>, text: &str) -> Report {
        Report {
            routine: RoutineId(0),
            run: RunId::new(),
            started: MONDAY + 9 * 3_600 + 5 * 60,
            finished: None,
            ending,
            text: text.into(),
            cost_usd: 0.0,
            read: false,
            branch: None,
            exchange: None,
        }
    }

    #[test]
    fn schedules_read_as_sentences() {
        let s = en();
        assert_eq!(schedule(&s, Schedule::Every { minutes: 60 }), "Every hour");
        assert_eq!(schedule(&s, Schedule::Every { minutes: 15 }), "Every 15 minutes");
        assert_eq!(schedule(&s, Schedule::Daily { minute: 9 * 60, utc_offset: 0 }), "Every day at 09:00");
        assert_eq!(
            schedule(&s, Schedule::Weekly { weekday: 4, minute: 17 * 60 + 30, utc_offset: 0 }),
            "Every Friday at 17:30"
        );
        let pt = Strings::new(Language::Portuguese);
        assert_eq!(schedule(&pt, Schedule::Daily { minute: 9 * 60, utc_offset: 0 }), "Todo dia às 09:00");
    }

    #[test]
    fn a_report_reads_its_time_ending_and_first_line() {
        let s = en();
        let done = report(Some(Ending::Completed), "\n## Two new issues\nOne is a regression.");
        assert_eq!(report_detail(&s, &done, MONDAY + 10 * 3_600, 0), "Today 09:05 · Two new issues");
        let running = report(None, "");
        assert_eq!(report_detail(&s, &running, MONDAY + 10 * 3_600, 0), "Today 09:05 · Running");
        let failed = report(Some(Ending::Failed("claude not found".into())), "");
        assert_eq!(report_detail(&s, &failed, MONDAY + 10 * 3_600, 0), "Today 09:05 · Failed · claude not found");
    }

    #[test]
    fn the_agents_closing_signal_names_the_report() {
        let s = en();
        let at = MONDAY + 10 * 3_600;
        let done = report(Some(Ending::Completed), "Checked the tests.\nresult: Two issues filed in Linear");
        assert_eq!(report_detail(&s, &done, at, 0), "Today 09:05 · Two issues filed in Linear");
        let blocked = report(Some(Ending::Completed), "Tried.\nneeds input: the Sentry token");
        assert_eq!(report_detail(&s, &blocked, at, 0), "Today 09:05 · Needs input · the Sentry token");
        let impossible = report(Some(Ending::Completed), "failed: no such repository");
        assert_eq!(report_detail(&s, &impossible, at, 0), "Today 09:05 · Failed · no such repository");
    }
}
