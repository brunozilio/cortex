use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_engine::model::http::Http;
use crossbeam_channel::{Receiver, Sender, TrySendError};
use domain::trace::runs::{Run, Status};
use domain::{EngineEvent, SessionId};
use platform::sync::Locked;

pub const ENDPOINT: &str = "OTLP_ENDPOINT";
pub const HEADERS: &str = "OTLP_HEADERS";
const QUEUED_AT_MOST: usize = 256;
const BATCH_WAIT: Duration = Duration::from_secs(2);
const BATCH_MOST: usize = 64;

pub type Traced = Arc<dyn Fn(&Path, &SessionId) -> Result<Vec<Run>, String> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Otlp {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl Otlp {
    pub fn from(lookup: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let endpoint = lookup(&brand::env(ENDPOINT)).map(|value| value.trim().trim_end_matches('/').to_owned());
        let endpoint = endpoint.filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
        let url = if endpoint.ends_with("/v1/traces") { endpoint } else { format!("{endpoint}/v1/traces") };
        let mut headers = vec![("content-type".to_owned(), "application/json".to_owned())];
        let listed = lookup(&brand::env(HEADERS)).unwrap_or_default();
        headers.extend(listed.split(',').filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_owned(), value.trim().to_owned()))
        }));
        Some(Self { url, headers })
    }
}

#[derive(Debug)]
struct Job {
    cwd: PathBuf,
    session: SessionId,
}

#[derive(Clone)]
pub struct Exporter {
    jobs: Sender<Job>,
    said: Arc<Said>,
}

impl std::fmt::Debug for Exporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exporter").finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub struct Said {
    once: std::sync::atomic::AtomicBool,
}

impl Said {
    pub fn failed(&self, problem: &str) -> bool {
        let first = !self.once.swap(true, std::sync::atomic::Ordering::Relaxed);
        if first {
            tracing::warn!(problem, "the trace could not be exported over OTLP; later failures are not logged");
        } else {
            tracing::debug!(problem, "the trace could not be exported over OTLP");
        }
        first
    }
}

impl Exporter {
    pub fn start(otlp: Otlp, traced: Traced, http: Arc<dyn Http>) -> Option<Self> {
        let (jobs, waiting) = crossbeam_channel::bounded(QUEUED_AT_MOST);
        let said = Arc::new(Said::default());
        let worker = Worker { otlp, traced, http, said: Arc::clone(&said), exported: HashMap::new() };
        let started = std::thread::Builder::new().name("otlp-export".into()).spawn(move || worker.serve(&waiting));
        match started {
            Ok(_) => Some(Self { jobs, said }),
            Err(error) => {
                said.failed(&error.to_string());
                None
            }
        }
    }

    pub fn finished(&self, cwd: &Path, session: &SessionId) {
        match self.jobs.try_send(Job { cwd: cwd.to_path_buf(), session: session.clone() }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.said.failed("too many runs are waiting to be exported");
            }
            Err(TrySendError::Disconnected(_)) => {
                self.said.failed("the exporter stopped");
            }
        }
    }

    pub fn watch(&self, cwd: &Path, resumed: Option<&SessionId>) -> Watch {
        Watch { exporter: self.clone(), cwd: cwd.to_path_buf(), session: Mutex::new(resumed.cloned()) }
    }
}

#[derive(Debug)]
pub struct Watch {
    exporter: Exporter,
    cwd: PathBuf,
    session: Mutex<Option<SessionId>>,
}

impl Watch {
    pub fn observe(&self, event: &EngineEvent) {
        if let EngineEvent::SessionStarted(id) = event {
            *self.session.locked() = Some(id.clone());
        }
        if event.is_terminal() {
            let session = self.session.locked().clone();
            if let Some(session) = session {
                self.exporter.finished(&self.cwd, &session);
            }
        }
    }
}

struct Worker {
    otlp: Otlp,
    traced: Traced,
    http: Arc<dyn Http>,
    said: Arc<Said>,
    exported: HashMap<String, usize>,
}

impl Worker {
    fn serve(mut self, waiting: &Receiver<Job>) {
        while let Ok(first) = waiting.recv() {
            let mut batch = vec![first];
            let deadline = Instant::now() + BATCH_WAIT;
            while batch.len() < BATCH_MOST {
                match waiting.recv_deadline(deadline) {
                    Ok(job) => batch.push(job),
                    Err(_) => break,
                }
            }
            let runs: Vec<Run> = batch.iter().flat_map(|job| self.fresh(job)).collect();
            if !runs.is_empty() {
                self.send(&runs);
            }
        }
    }

    fn fresh(&mut self, job: &Job) -> Vec<Run> {
        let runs = match (self.traced)(&job.cwd, &job.session) {
            Ok(runs) => runs,
            Err(problem) => {
                tracing::debug!(%problem, "a finished run's trace could not be read for export");
                return Vec::new();
            }
        };
        let done: Vec<Run> = runs.into_iter().filter(|run| run.status != Status::Running).collect();
        let Some(last) = done.last().map(|run| run.index) else { return Vec::new() };
        let from = self.exported.insert(job.session.0.clone(), last + 1).unwrap_or(last);
        done.into_iter().filter(|run| run.index >= from).collect()
    }

    fn send(&self, runs: &[Run]) {
        let body = domain::trace::otlp::request(runs, env!("CARGO_PKG_VERSION")).to_string();
        match self.http.post(&self.otlp.url, &self.otlp.headers, &body) {
            Ok(answer) if (200..300).contains(&answer.status) => {}
            Ok(answer) => {
                self.said.failed(&format!("{} answered {}", self.otlp.url, answer.status));
            }
            Err(error) => {
                self.said.failed(&format!("{}: {error}", self.otlp.url));
            }
        }
    }
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
