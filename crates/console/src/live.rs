use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::stream::{self, Stream, StreamExt};
use serde_json::Value;
use tokio::sync::broadcast;

use crate::routes::{Shared, failure};

const KEPT_UPDATES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub session: String,
    pub text: String,
}

#[derive(Debug, Default)]
struct Board {
    report: Option<String>,
    docs: Option<String>,
    sessions: Option<String>,
    latest: BTreeMap<String, String>,
    prompts: Vec<Prompt>,
    watched: Option<String>,
}

#[derive(Debug, Clone)]
enum Update {
    Session(String),
    Report,
}

impl Update {
    fn event(self) -> Event {
        match self {
            Self::Session(line) => Event::default().data(line),
            Self::Report => Event::default().event("report").data("changed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Live {
    board: Arc<Mutex<Board>>,
    updates: broadcast::Sender<Update>,
}

impl Default for Live {
    fn default() -> Self {
        Self { board: Arc::default(), updates: broadcast::channel(KEPT_UPDATES).0 }
    }
}

impl Live {
    fn board(&self) -> MutexGuard<'_, Board> {
        self.board.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn show(&self, report: String) {
        self.board().report = Some(report);
        let _ = self.updates.send(Update::Report);
    }

    pub fn document(&self, docs: String) {
        self.board().docs = Some(docs);
    }

    pub fn list(&self, sessions: String) {
        self.board().sessions = Some(sessions);
    }

    pub fn send(&self, session: &str, data: &str) {
        let line = data.replace(['\r', '\n'], " ");
        self.board().latest.insert(session.to_owned(), line.clone());
        let _ = self.updates.send(Update::Session(line));
    }

    pub fn take_prompts(&self) -> Vec<Prompt> {
        std::mem::take(&mut self.board().prompts)
    }

    pub fn watched(&self) -> Option<String> {
        self.board().watched.clone()
    }

    pub fn folder_of(&self, session: &str) -> Option<PathBuf> {
        let listed: Value = serde_json::from_str(self.board().sessions.as_deref()?).ok()?;
        let card = listed.as_array()?.iter().find(|card| card["session"].as_str() == Some(session))?;
        Some(PathBuf::from(card["cwd"].as_str()?)).filter(|folder| folder.is_dir())
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/report", get(report))
        .route("/api/docs", get(docs))
        .route("/api/sessions", get(sessions))
        .route("/api/events", get(events))
        .route("/api/prompt", post(prompt))
        .route("/api/watch", post(watch))
}

fn json(body: String) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn report(State(shared): State<Shared>) -> Response {
    match shared.live.board().report.clone() {
        Some(report) => json(report),
        None => failure(StatusCode::SERVICE_UNAVAILABLE, "not_ready", "The terminal has not sent its report yet."),
    }
}

async fn docs(State(shared): State<Shared>) -> Response {
    match shared.live.board().docs.clone() {
        Some(docs) => json(docs),
        None => failure(StatusCode::SERVICE_UNAVAILABLE, "not_ready", "The terminal has not sent the docs yet."),
    }
}

async fn sessions(State(shared): State<Shared>) -> Response {
    json(shared.live.board().sessions.clone().unwrap_or_else(|| "[]".into()))
}

async fn events(State(shared): State<Shared>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = shared.live.updates.subscribe();
    let latest: Vec<Update> = shared.live.board().latest.values().cloned().map(Update::Session).collect();
    let following = stream::unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(update) => return Some((update, receiver)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    let updates = stream::iter(latest).chain(following);
    Sse::new(updates.map(|update| Ok(update.event()))).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

fn named(body: &Value, field: &str) -> Option<String> {
    body[field].as_str().map(str::trim).filter(|text| !text.is_empty()).map(str::to_owned)
}

async fn prompt(State(shared): State<Shared>, axum::Json(body): axum::Json<Value>) -> Response {
    let (Some(session), Some(text)) = (named(&body, "session"), named(&body, "text")) else {
        return failure(StatusCode::BAD_REQUEST, "validation", "A prompt needs a session and some text.");
    };
    shared.live.board().prompts.push(Prompt { session, text });
    StatusCode::ACCEPTED.into_response()
}

async fn watch(State(shared): State<Shared>, axum::Json(body): axum::Json<Value>) -> Response {
    let Some(session) = named(&body, "session") else {
        return failure(StatusCode::BAD_REQUEST, "validation", "Say which session to watch.");
    };
    shared.live.board().watched = Some(session);
    StatusCode::ACCEPTED.into_response()
}
