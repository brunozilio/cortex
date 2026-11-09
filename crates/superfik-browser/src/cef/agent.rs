//! An agent's operations on one page, step by step: a navigation waits for
//! the load, a click on a ref first asks the page where it is, a
//! screenshot waits for Chromium's capture. One operation runs at a time;
//! the rest queue behind it.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use base64::Engine;
use superfik_core::{BrowseId, BrowserAnswer, BrowserOp, Locator};
use welding::{CefSurfaceProducer, SnapshotRequestId};

use super::automation::Operation;
use super::{Page, log};
use crate::agent;
use crate::{Modifiers, PageInput, address};
mod start;

const LOAD_LIMIT: Duration = Duration::from_secs(20);
const LOAD_START: Duration = Duration::from_secs(2);
const SETTLE: Duration = Duration::from_millis(500);
const LEAD: Duration = Duration::from_secs(3);
const SCRIPT_LIMIT: Duration = Duration::from_secs(10);

enum Step {
    Done(Result<BrowserAnswer, String>),
    Wait(super::waiting::Waiting),
    Load { started: bool, revision: u64, start_by: Instant, deadline: Instant },
    Native { deadline: Instant, action: bool },
    Shot { id: SnapshotRequestId, deadline: Instant },
    Settle { until: Instant, text: String, started: bool, lead_by: Instant },
}

struct Job {
    request: BrowseId,
    step: Step,
}

#[derive(Default)]
pub(super) struct Jobs {
    queue: VecDeque<(BrowseId, BrowserOp)>,
    current: Option<Job>,
}

type Answer = (BrowseId, Result<BrowserAnswer, String>);

impl Page {
    pub(super) fn queue(&mut self, request: BrowseId, op: BrowserOp) {
        self.jobs.queue.push_back((request, op));
    }

    pub(super) fn advance(&mut self, now: Instant) -> Vec<Answer> {
        let mut answers = Vec::new();
        loop {
            if self.jobs.current.is_none() {
                let Some((request, op)) = self.jobs.queue.pop_front() else { break };
                match self.start(op, now) {
                    Ok(step) => self.jobs.current = Some(Job { request, step }),
                    Err(error) => answers.push((request, Err(error))),
                }
                continue;
            }
            match self.step(now) {
                Some(answer) => answers.push(answer),
                None => break,
            }
        }
        answers
    }

    fn step(&mut self, now: Instant) -> Option<Answer> {
        let (summary, loading, failure) = (self.summary(), self.loading, self.failure.clone());
        let job = self.jobs.current.as_mut()?;
        let done = match &mut job.step {
            Step::Done(answer) => Some(answer.clone()),
            Step::Wait(wait) => wait.poll(&mut self.automation, loading, &self.url, now),
            Step::Load { started, revision, start_by, deadline } => {
                *started |= loading || self.load_revision > *revision;
                if self.automation.has_dialog() {
                    Some(Ok(self.automation.dialog_summary()))
                } else if let Some(error) = failure {
                    Some(Err(format!("The page failed to load: {error}")))
                } else if *started && !loading {
                    Some(Ok(BrowserAnswer::text(summary.clone())))
                } else if now >= *deadline {
                    Some(Err(format!("Navigation timed out.\n{summary}")))
                } else if !*started && now >= *start_by {
                    Some(Ok(BrowserAnswer::text(summary.clone())))
                } else {
                    None
                }
            }
            Step::Settle { until, text, started, lead_by } => {
                *started |= loading;
                let leading = !*started && now < *lead_by;
                let waiting = now < *until || leading || (*started && loading && now < *until + LOAD_LIMIT);
                if let Some(error) = failure {
                    Some(Err(format!("The page failed to load: {error}")))
                } else if loading && now >= *until + LOAD_LIMIT {
                    Some(Err(format!("Navigation timed out after the action.\n{summary}")))
                } else {
                    (!waiting).then(|| Ok(BrowserAnswer::text(format!("{text}\n{summary}"))))
                }
            }
            Step::Native { deadline, action } => match self.automation.answer.take() {
                Some(Ok(answer)) if *action => {
                    job.step = settle(answer.text, now);
                    None
                }
                Some(answer) => Some(answer),
                None if now >= *deadline => {
                    self.automation.cancel();
                    Some(Err("Native browser operation timed out.".into()))
                }
                None => None,
            },
            Step::Shot { id, deadline } => match self.producer.poll_snapshot_png() {
                Some(shot) if shot.id == *id => Some(match shot.result {
                    Ok(png) => Ok(BrowserAnswer {
                        text: summary.clone(),
                        png: Some(base64::engine::general_purpose::STANDARD.encode(png)),
                    }),
                    Err(error) => Err(format!("Could not capture the page: {error}")),
                }),
                _ if now >= *deadline => Some(Err("The capture did not arrive.".to_owned())),
                _ => None,
            },
        };
        let answer = done?;
        let job = self.jobs.current.take()?;
        Some((job.request, answer))
    }

    fn summary(&self) -> String {
        format!("Title: {}\nURL: {}", self.title, self.url)
    }

    pub(super) fn close_jobs(&mut self, completed_action: bool) -> Vec<Answer> {
        let mut answers = Vec::new();
        if let Some(job) = self.jobs.current.take() {
            let answer =
                if completed_action && matches!(job.step, Step::Native { action: true, .. } | Step::Settle { .. }) {
                    Ok(BrowserAnswer::text("Popup closed."))
                } else {
                    Err("The popup closed before the operation completed.".into())
                };
            answers.push((job.request, answer));
        }
        answers.extend(self.jobs.queue.drain(..).map(|(request, _)| (request, Err("The popup closed.".into()))));
        answers
    }
}

fn settle(text: String, now: Instant) -> Step {
    Step::Settle { until: now + SETTLE, text, started: false, lead_by: now }
}

fn lead(text: String, now: Instant) -> Step {
    Step::Settle { until: now + SETTLE, text, started: false, lead_by: now + LEAD }
}
