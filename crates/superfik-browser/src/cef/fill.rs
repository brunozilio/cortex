//! A login fill on one page, over the DevTools protocol and in our isolated
//! world: list the page's frames, read the fields of the top one and of
//! every frame on the same site (or an equivalent one), choose where the
//! username and the password go (`superfik_core::autofill::plan`, the
//! frame holding the focus first, then the top one), then place them while
//! that frame still shows the address it was read at. Frames of other
//! sites are never filled; the ones the browser runs in another process
//! (cross-site) are not even listed. One fill at a time; a new one replaces
//! the one under way.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde_json::Value;
use superfik_core::autofill::{self, FillOutcome, FillPlan, LoginFill, LoginUri};
use welding::CefSurfaceProducer;

use super::{Page, log};
use crate::autofill::{fill_outcome, fill_script, read_page, read_script};
use crate::devtools;

const LIMIT: Duration = Duration::from_secs(8);

enum Wait {
    Frames(u64),
    World(u64),
    Read(u64, i64),
    Fill(u64),
}

struct Found {
    context: i64,
    plan: FillPlan,
    url: String,
    focused: bool,
}

pub(super) struct Filling {
    url: String,
    fill: LoginFill,
    deadline: Instant,
    wait: Wait,
    frames: VecDeque<String>,
    found: Vec<Found>,
}

impl Page {
    pub(super) fn start_fill(&mut self, url: String, fill: LoginFill) {
        if self.filling.take().is_some() {
            self.fill_answers.push(FillOutcome::Denied);
        }
        if !autofill::same_document(&url, &self.url) {
            return self.fill_answers.push(FillOutcome::Denied);
        }
        let (id, message) = self.cdp.frame_tree();
        if !self.send_devtools(&message) {
            return self.fill_answers.push(FillOutcome::Denied);
        }
        let deadline = Instant::now() + LIMIT;
        self.filling =
            Some(Filling { url, fill, deadline, wait: Wait::Frames(id), frames: VecDeque::new(), found: Vec::new() });
    }

    pub(super) fn fill_reply(&mut self, id: u64, result: Result<Value, String>) {
        let Some(mut filling) = self.filling.take() else { return };
        let waited = match filling.wait {
            Wait::Frames(w) | Wait::World(w) | Wait::Read(w, _) | Wait::Fill(w) => w,
        };
        if waited != id {
            self.filling = Some(filling);
            return;
        }
        let outcome = match (&filling.wait, result) {
            (Wait::Fill(_), Ok(reply)) => {
                Some(devtools::value(&reply).map_or(FillOutcome::Denied, |v| fill_outcome(&v)))
            }
            (Wait::Frames(_), Ok(reply)) => match eligible(&filling.url, &devtools::frames(&reply)) {
                Some(frames) => {
                    filling.frames = frames;
                    self.next_frame(&mut filling)
                }
                None => Some(FillOutcome::Denied),
            },
            (Wait::World(_), Ok(reply)) => match devtools::context(&reply) {
                Some(context) => {
                    let (id, message) = self.cdp.evaluate(context, &read_script());
                    filling.wait = Wait::Read(id, context);
                    (!self.send_devtools(&message)).then_some(FillOutcome::Denied)
                }
                None => self.next_frame(&mut filling),
            },
            (Wait::Read(_, context), Ok(reply)) => {
                let context = *context;
                if let Some(page) = devtools::value(&reply).ok().and_then(|json| read_page(&json).ok()) {
                    let focused = page.fields.iter().any(|f| f.focused);
                    if let Some(plan) = autofill::plan(&page.fields) {
                        filling.found.push(Found { context, plan, url: page.url, focused });
                    }
                }
                self.next_frame(&mut filling)
            }
            (Wait::World(_) | Wait::Read(..), Err(_)) => self.next_frame(&mut filling),
            (Wait::Frames(_) | Wait::Fill(_), Err(error)) => {
                tracing::warn!(%error, "a login fill failed");
                Some(FillOutcome::Denied)
            }
        };
        match outcome {
            Some(outcome) => self.fill_answers.push(outcome),
            None => self.filling = Some(filling),
        }
    }

    fn next_frame(&mut self, filling: &mut Filling) -> Option<FillOutcome> {
        if let Some(frame) = filling.frames.pop_front() {
            let (id, message) = self.cdp.isolated_world(&frame);
            filling.wait = Wait::World(id);
            return (!self.send_devtools(&message)).then_some(FillOutcome::Denied);
        }
        let best = filling.found.iter().find(|f| f.focused).or_else(|| filling.found.first());
        let Some(best) = best else { return Some(FillOutcome::Absent) };
        let Ok(at) = url::Url::parse(&best.url) else { return Some(FillOutcome::Denied) };
        let script = fill_script(best.plan, &filling.fill, &at.origin().ascii_serialization(), at.path());
        let (id, message) = self.cdp.evaluate(best.context, &script);
        filling.wait = Wait::Fill(id);
        (!self.send_devtools(&message)).then_some(FillOutcome::Denied)
    }

    pub(super) fn fill_overdue(&mut self, now: Instant) -> Option<FillOutcome> {
        let overdue = self.filling.as_ref().is_some_and(|filling| now >= filling.deadline);
        overdue.then(|| {
            self.filling = None;
            FillOutcome::Denied
        })
    }

    pub(super) fn send_devtools(&mut self, message: &str) -> bool {
        let sent = self.producer.send_devtools_message(message);
        let ok = sent.is_ok();
        log(sent);
        ok
    }
}

fn eligible(chosen: &str, frames: &[(String, String)]) -> Option<VecDeque<String>> {
    let (top, top_url) = frames.first()?;
    if !autofill::same_document(chosen, top_url) {
        return None;
    }
    let site = LoginUri::new(top_url.clone(), None);
    let children = frames[1..].iter().filter(|(_, url)| site.matches(url)).map(|(id, _)| id.clone());
    Some(std::iter::once(top.clone()).chain(children).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(id, url)| ((*id).to_owned(), (*url).to_owned())).collect()
    }

    #[test]
    fn same_site_frames_are_read_after_the_top_one_and_others_never() {
        let page = frames(&[
            ("top", "https://www.example.com/account"),
            ("login", "https://login.example.com/form"),
            ("ads", "https://ads.tracker.net/frame"),
        ]);
        let chosen = eligible("https://www.example.com/account", &page).expect("still the page");
        assert_eq!(chosen, ["top", "login"]);
        assert!(eligible("https://www.example.com/other", &page).is_none(), "the page moved on");
        assert!(eligible("https://www.example.com/account", &[]).is_none());
    }
}
