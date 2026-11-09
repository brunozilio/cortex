//! The Browser pane's rows: one per browser found, whose button brings its
//! logins over (Importar → Importando → Importado), and one that clears
//! the cards' cookies (Limpar → Limpo). Pure; `now` places the animations
//! of what just started or finished.

use superfik_browser::import::BrowserKind;
use superfik_i18n::{Strings, Text};
use superfik_ui::components::Progress;
use superfik_ui::icons;

use super::rows::{Control, Group, Line, Run};
use crate::state::Logins;

pub fn groups(logins: &Logins, from: Option<BrowserKind>, importable: bool, strings: &Strings, now: f64) -> Vec<Group> {
    let t = |text: Text<'_>| strings.t(text);
    let age = |since: f64| (now - since).max(0.0) as f32;
    let import = [t(Text::LoginsImport), t(Text::LoginsImporting), t(Text::LoginsImported)];
    let browsers = if logins.sources.is_empty() {
        vec![Line {
            browser: None,
            title: t(Text::LoginsNoBrowser),
            detail: Some(t(Text::LoginsNoBrowserCaption)),
            control: Control::Action { labels: import, icon: icons::IMPORT, run: None, progress: Progress::Idle },
        }]
    } else {
        logins
            .sources
            .iter()
            .map(|&kind| {
                let in_use = from == Some(kind);
                let in_use_caption = in_use.then(|| t(Text::LoginsInUse));
                let (progress, detail) = match (&logins.importing, &logins.outcome) {
                    (Some((reading, since)), _) if *reading == kind => {
                        (Progress::Busy { age: age(*since), time: now as f32 }, None)
                    }
                    (_, Some((done, Ok(at)))) if *done == kind => {
                        (Progress::Done { age: age(*at), after_busy: true }, in_use_caption)
                    }
                    (_, Some((done, Err(error)))) if *done == kind => (Progress::Idle, Some(error.clone())),
                    _ => (Progress::Idle, in_use_caption),
                };
                let mut labels = import.clone();
                if in_use {
                    labels[0] = t(Text::LoginsImportAgain);
                }
                let run = logins.importing.is_none().then_some(Run::ImportLogins(kind));
                let control = Control::Action { labels, icon: icons::IMPORT, run, progress };
                Line { browser: Some(kind), title: kind.name().to_owned(), detail, control }
            })
            .collect()
    };
    let cleared = match logins.cleared {
        Some(at) => Progress::Done { age: age(at), after_busy: false },
        None => Progress::Idle,
    };
    let clear = Line {
        browser: None,
        title: t(Text::ClearCookies),
        detail: Some(t(Text::ClearCookiesCaption)),
        control: Control::Action {
            labels: [t(Text::ClearCookiesButton), t(Text::ClearingCookies), t(Text::CookiesCleared)],
            icon: icons::DELETE,
            run: Some(Run::ClearCookies),
            progress: cleared,
        },
    };
    let clearing = Group { heading: None, lines: vec![clear] };
    if !importable {
        return vec![clearing];
    }
    vec![Group { heading: Some(t(Text::SettingsLogins)), lines: browsers }, clearing]
}

#[cfg(test)]
mod tests {
    use superfik_i18n::Language;
    use superfik_ui::theme::motion;

    use super::*;

    fn browsers(logins: &Logins, from: Option<BrowserKind>, now: f64) -> Vec<(String, Option<String>, Progress, bool)> {
        let groups = groups(logins, from, true, &Strings::new(Language::English), now);
        groups[0]
            .lines
            .iter()
            .map(|line| match &line.control {
                Control::Action { run, progress, .. } => {
                    (line.title.clone(), line.detail.clone(), *progress, run.is_some())
                }
                _ => unreachable!("browsers have buttons"),
            })
            .collect()
    }

    #[test]
    fn the_browser_in_use_can_be_imported_again() {
        let logins = Logins { sources: vec![BrowserKind::Dia, BrowserKind::Chrome], ..Logins::default() };
        let groups = groups(&logins, Some(BrowserKind::Dia), true, &Strings::new(Language::English), 0.0);
        let dia = &groups[0].lines[0];
        assert_eq!(dia.detail.as_deref(), Some("New sessions start with these logins."));
        let Control::Action { labels, run, progress, .. } = &dia.control else { unreachable!("a button") };
        assert_eq!(
            (labels[0].as_str(), *run, *progress),
            ("Import again", Some(Run::ImportLogins(BrowserKind::Dia)), Progress::Idle)
        );
        assert_eq!(browsers(&logins, Some(BrowserKind::Dia), 0.0)[1], ("Chrome".into(), None, Progress::Idle, true));
    }

    #[test]
    fn a_good_import_shows_done_for_a_moment_then_can_be_imported_again() {
        let logins = Logins {
            sources: vec![BrowserKind::Dia],
            outcome: Some((BrowserKind::Dia, Ok(10.0))),
            ..Logins::default()
        };
        let row = &browsers(&logins, Some(BrowserKind::Dia), 11.0)[0];
        assert_eq!(row.2, Progress::Done { age: 1.0, after_busy: true });
        assert_eq!(row.1.as_deref(), Some("New sessions start with these logins."));
        let rest = 10.0 + f64::from(motion::STATUS_DONE_HOLD + motion::STATUS_REVERT);
        assert!(!browsers(&logins, Some(BrowserKind::Dia), rest)[0].2.is_moving());
        assert!(logins.is_animating(rest - 0.1) && !logins.is_animating(rest));
    }

    #[test]
    fn an_import_goes_busy_then_done() {
        let mut logins = Logins {
            sources: vec![BrowserKind::Dia, BrowserKind::Chrome],
            importing: Some((BrowserKind::Chrome, 10.0)),
            ..Logins::default()
        };
        let rows = browsers(&logins, None, 10.5);
        assert_eq!(rows[1].2, Progress::Busy { age: 0.5, time: 10.5 });
        assert!(rows.iter().all(|row| !row.3), "one import at a time");
        logins.importing = None;
        logins.outcome = Some((BrowserKind::Chrome, Ok(12.0)));
        assert_eq!(
            browsers(&logins, Some(BrowserKind::Chrome), 12.25)[1].2,
            Progress::Done { age: 0.25, after_busy: true }
        );
        logins.outcome = Some((BrowserKind::Chrome, Err("macOS said no.".into())));
        assert_eq!(browsers(&logins, None, 13.0)[1].1.as_deref(), Some("macOS said no."));
    }

    #[test]
    fn cookies_can_be_cleared_and_say_so() {
        let logins = Logins { cleared: Some(1.0), ..Logins::default() };
        let groups = groups(&logins, None, true, &Strings::new(Language::English), 1.5);
        let line = &groups[1].lines[0];
        assert_eq!(line.title, "Clear cookies");
        let Control::Action { labels, run, progress, .. } = &line.control else { unreachable!("a button") };
        assert_eq!(labels[2], "Cleared");
        assert_eq!((*run, *progress), (Some(Run::ClearCookies), Progress::Done { age: 0.5, after_busy: false }));
        let rest = 1.0 + f64::from(motion::STATUS_DONE_HOLD + motion::STATUS_REVERT);
        assert!(logins.is_animating(rest - 0.1) && !logins.is_animating(rest), "back to Clear by itself");
        assert_eq!(groups[0].lines[0].title, "No browser to import from");
    }

    #[test]
    fn a_browser_tab_only_clears_cookies() {
        let logins = Logins { sources: vec![BrowserKind::Chrome], ..Logins::default() };
        let groups = groups(&logins, None, false, &Strings::new(Language::English), 0.0);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].lines[0].title, "Clear cookies");
    }
}
