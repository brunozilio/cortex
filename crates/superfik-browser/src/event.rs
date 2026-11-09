//! What a page reports back to the app.

use superfik_core::page::{PageCursor, PageReport};
use superfik_core::{BrowseId, BrowserAnswer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageEvent {
    Address(String),
    Title(String),
    Loading(bool),
    History { back: bool, forward: bool },
    Failed { url: String, error: String },
    Crashed,
    Copied(String),
    Cursor(PageCursor),
    LoginFilled(superfik_core::autofill::FillOutcome),
    LoginSubmitted(crate::capture::Submitted),
    PasskeyRequested(crate::passkey::PasskeyAsk),
    Answered { request: BrowseId, answer: Result<BrowserAnswer, String> },
}

impl PageEvent {
    pub fn report(self) -> Option<PageReport> {
        Some(match self {
            Self::Address(url) => PageReport::Address(url),
            Self::Title(title) => PageReport::Title(title),
            Self::Loading(loading) => PageReport::Loading(loading),
            Self::History { back, forward } => PageReport::History { back, forward },
            Self::Failed { url, error } => PageReport::Failed { url, error },
            Self::Crashed => PageReport::Crashed,
            Self::Copied(text) => PageReport::Copied(text),
            Self::Cursor(cursor) => PageReport::Cursor(cursor),
            Self::Answered { request, answer } => PageReport::Answered { request, answer },
            Self::LoginFilled(_) | Self::LoginSubmitted(_) | Self::PasskeyRequested(_) => return None,
        })
    }
}

impl From<PageReport> for PageEvent {
    fn from(report: PageReport) -> Self {
        match report {
            PageReport::Address(url) => Self::Address(url),
            PageReport::Title(title) => Self::Title(title),
            PageReport::Loading(loading) => Self::Loading(loading),
            PageReport::History { back, forward } => Self::History { back, forward },
            PageReport::Failed { url, error } => Self::Failed { url, error },
            PageReport::Crashed => Self::Crashed,
            PageReport::Copied(text) => Self::Copied(text),
            PageReport::Cursor(cursor) => Self::Cursor(cursor),
            PageReport::Answered { request, answer } => Self::Answered { request, answer },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_comes_back_as_the_event_it_was() {
        let event = PageEvent::History { back: true, forward: false };
        assert_eq!(PageEvent::from(event.clone().report().expect("travels")), event);
        let filled = PageEvent::LoginFilled(superfik_core::autofill::FillOutcome::Filled);
        assert_eq!(filled.report(), None, "sign-ins stay on the server");
    }
}
