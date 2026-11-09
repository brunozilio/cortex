//! Poll only while an explicit agent wait is active; never throttle the page.
use super::automation::{Automation, Operation};
use std::time::{Duration, Instant};
use superfik_core::{BrowserAnswer, browser::BrowserWait};

pub(super) struct Waiting {
    query: BrowserWait,
    deadline: Instant,
    next_poll: Instant,
    reading: bool,
}
impl Waiting {
    pub fn new(query: BrowserWait, now: Instant) -> Result<Self, String> {
        query.validate()?;
        let deadline = now + Duration::from_millis(query.timeout_ms.into());
        Ok(Self { query, deadline, next_poll: now, reading: false })
    }
    pub fn poll(
        &mut self,
        automation: &mut Automation,
        loading: bool,
        url: &str,
        now: Instant,
    ) -> Option<Result<BrowserAnswer, String>> {
        if automation.has_dialog() {
            automation.cancel();
            return Some(Err("A JavaScript dialog needs an explicit browser_dialog decision.".into()));
        }
        if now >= self.deadline {
            automation.cancel();
            return Some(Err("Browser wait timed out: the requested condition was not met.".into()));
        }
        let needs_snapshot = self.query.text.is_some() || self.query.reference.is_some();
        if self.reading {
            match automation.answer.take()? {
                Ok(_) => {
                    self.reading = false;
                    let text = self.query.text.as_ref().is_none_or(|t| automation.contains(t));
                    let element = self.query.reference.is_none_or(|r| automation.visible(r) != self.query.hidden);
                    let address = self.query.url.as_ref().is_none_or(|u| url.contains(u));
                    if text && element && address {
                        return Some(Ok(BrowserAnswer::text(format!("Wait condition satisfied.\nURL: {url}"))));
                    }
                }
                Err(error) if !loading => return Some(Err(error)),
                Err(_) => self.reading = false,
            }
            self.next_poll = now + Duration::from_millis(100);
        }
        if !needs_snapshot && !url.is_empty() && !loading && self.query.url.as_ref().is_none_or(|u| url.contains(u)) {
            return Some(Ok(BrowserAnswer::text(format!("Page ready.\nURL: {url}"))));
        }
        if needs_snapshot && now >= self.next_poll {
            if let Err(error) = automation.begin(Operation::Snapshot(0)) {
                return Some(Err(error));
            }
            self.reading = true;
        }
        None
    }
}
