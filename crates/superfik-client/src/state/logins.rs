//! Logins brought over from the user's own browser, from Settings ›
//! Browser: which browsers there are, the import under way and how the
//! last one went, and clearing the cards' cookies. A successful import is
//! remembered in the preferences, so sessions opened later start signed in
//! (the shell does the reading). Times are kept so the screen can animate
//! the moment something starts or finishes.

use superfik_browser::import::BrowserKind;
use superfik_ui::theme::motion;

use super::{Effect, Session};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Logins {
    pub sources: Vec<BrowserKind>,
    pub importing: Option<(BrowserKind, f64)>,
    pub outcome: Option<(BrowserKind, Result<f64, String>)>,
    pub cleared: Option<f64>,
}

impl Logins {
    pub fn is_animating(&self, now: f64) -> bool {
        let settling = |at: f64| now - at < f64::from(motion::STATUS_DONE_HOLD + motion::STATUS_REVERT);
        self.importing.is_some()
            || matches!(self.outcome, Some((_, Ok(at))) if settling(at))
            || self.cleared.is_some_and(settling)
    }
}

impl Session {
    pub(super) fn import_logins(&mut self, kind: BrowserKind, now: f64) -> Vec<Effect> {
        if !self.platform.import_logins || self.logins.importing.is_some() || !self.logins.sources.contains(&kind) {
            return Vec::new();
        }
        self.logins.importing = Some((kind, now));
        self.logins.outcome = None;
        vec![Effect::ImportLogins(kind)]
    }

    pub(super) fn logins_imported(
        &mut self,
        kind: BrowserKind,
        result: Result<usize, String>,
        now: f64,
    ) -> Vec<Effect> {
        self.logins.importing = None;
        let imported = result.is_ok();
        self.logins.outcome = Some((kind, result.map(|_| now)));
        if imported {
            self.logins.cleared = None;
            if self.preferences.logins != Some(kind) {
                self.preferences.logins = Some(kind);
                return vec![Effect::Persist];
            }
        }
        Vec::new()
    }

    pub(super) fn clear_browser_cookies(&mut self, now: f64) -> Vec<Effect> {
        self.logins.cleared = Some(now);
        self.logins.outcome = None;
        let forgot = self.preferences.logins.take().is_some();
        let mut effects = vec![Effect::ClearCookies];
        if forgot {
            effects.push(Effect::Persist);
        }
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Action;

    fn session() -> Session {
        let mut s = Session::default();
        s.apply(Action::BrowserSources(vec![BrowserKind::Dia, BrowserKind::Chrome]), 0.0);
        s
    }

    #[test]
    fn importing_reads_one_browser_at_a_time() {
        let mut s = session();
        let effects = s.apply(Action::ImportLogins(BrowserKind::Dia), 1.0);
        assert!(matches!(effects.first(), Some(Effect::ImportLogins(BrowserKind::Dia))), "{effects:?}");
        assert_eq!(s.logins.importing, Some((BrowserKind::Dia, 1.0)));
        let again = s.apply(Action::ImportLogins(BrowserKind::Chrome), 1.0);
        assert!(!again.iter().any(|e| matches!(e, Effect::ImportLogins(_))), "one at a time");
        let missing = Session::default().apply(Action::ImportLogins(BrowserKind::Arc), 0.0);
        assert!(!missing.iter().any(|e| matches!(e, Effect::ImportLogins(_))), "not installed");
    }

    #[test]
    fn a_good_import_is_remembered_and_a_failed_one_is_not() {
        let mut s = session();
        s.apply(Action::ImportLogins(BrowserKind::Dia), 0.0);
        let effects = s.apply(Action::LoginsImported(BrowserKind::Dia, Ok(120)), 2.0);
        assert!(matches!(effects.first(), Some(Effect::Persist)), "{effects:?}");
        assert_eq!((s.preferences.logins, s.logins.importing), (Some(BrowserKind::Dia), None));
        assert_eq!(s.logins.outcome, Some((BrowserKind::Dia, Ok(2.0))), "when, for the check to appear");
        s.apply(Action::ImportLogins(BrowserKind::Chrome), 3.0);
        let failed = s.apply(Action::LoginsImported(BrowserKind::Chrome, Err("denied".into())), 4.0);
        assert!(!failed.iter().any(|e| matches!(e, Effect::Persist)));
        assert_eq!(s.preferences.logins, Some(BrowserKind::Dia), "keeps the browser that worked");
        assert_eq!(s.logins.outcome, Some((BrowserKind::Chrome, Err("denied".into()))));
    }

    #[test]
    fn the_pane_animates_while_reading_and_until_the_check_settles() {
        let mut s = session();
        assert!(!s.logins.is_animating(0.0));
        s.apply(Action::ImportLogins(BrowserKind::Dia), 0.0);
        assert!(s.logins.is_animating(30.0), "however long macOS waits");
        s.apply(Action::LoginsImported(BrowserKind::Dia, Ok(3)), 31.0);
        assert!(s.logins.is_animating(31.2));
        assert!(s.logins.is_animating(33.0), "done until it is back at rest");
        assert!(!s.logins.is_animating(31.0 + f64::from(motion::STATUS_DONE_HOLD + motion::STATUS_REVERT)), "settled");
    }

    #[test]
    fn clearing_signs_out_and_forgets_the_logins() {
        let mut s = session();
        s.apply(Action::ImportLogins(BrowserKind::Dia), 0.0);
        s.apply(Action::LoginsImported(BrowserKind::Dia, Ok(3)), 1.0);
        let effects = s.apply(Action::ClearBrowserCookies, 5.0);
        assert!(matches!(effects[..2], [Effect::ClearCookies, Effect::Persist]), "{effects:?}");
        assert_eq!((s.preferences.logins, s.logins.cleared, &s.logins.outcome), (None, Some(5.0), &None));
        let again = s.apply(Action::ClearBrowserCookies, 6.0);
        assert!(!again.iter().any(|e| matches!(e, Effect::Persist)), "nothing more to forget");
    }
}
