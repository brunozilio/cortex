use std::sync::{Arc, PoisonError, RwLock};

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};

use crate::routes::{Shared, failure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatError {
    Missing(String),
    Busy(String),
    Failed(String),
}

pub trait Chats: Send + Sync {
    fn list(&self) -> Result<Value, ChatError>;
    fn open(&self, session: &str) -> Result<Value, ChatError>;
    fn start(&self, text: &str) -> Result<String, ChatError>;
    fn send(&self, session: &str, text: &str) -> Result<(), ChatError>;
    fn answer(&self, session: &str, ask: &str, decision: &str) -> Result<(), ChatError>;
    fn stop(&self, session: &str) -> Result<(), ChatError>;
}

#[derive(Clone, Default)]
pub struct Desk(Arc<RwLock<Option<Arc<dyn Chats>>>>);

impl std::fmt::Debug for Desk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(if self.current().is_some() { "Desk(open)" } else { "Desk(closed)" })
    }
}

impl Desk {
    pub fn open(&self, chats: Arc<dyn Chats>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = Some(chats);
    }

    fn current(&self) -> Option<Arc<dyn Chats>> {
        self.0.read().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

pub const MOST_PROMPT_BYTES: usize = 32 * 1024;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/chats", get(list).post(start))
        .route("/api/chats/{session}", get(open))
        .route("/api/chats/{session}/messages", post(send))
        .route("/api/chats/{session}/answer", post(answer))
        .route("/api/chats/{session}/stop", post(stop))
}

fn answered(result: Result<Value, ChatError>) -> Response {
    match result {
        Ok(body) => ([(header::CONTENT_TYPE, "application/json")], body.to_string()).into_response(),
        Err(ChatError::Missing(told)) => failure(StatusCode::NOT_FOUND, "not_found", &told),
        Err(ChatError::Busy(told)) => failure(StatusCode::CONFLICT, "busy", &told),
        Err(ChatError::Failed(told)) => failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", &told),
    }
}

async fn on_desk(
    shared: &Shared,
    work: impl FnOnce(&dyn Chats) -> Result<Value, ChatError> + Send + 'static,
) -> Response {
    let Some(chats) = shared.live.desk().current() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "not_ready", "The terminal has not opened its chats yet.");
    };
    match tokio::task::spawn_blocking(move || work(chats.as_ref())).await {
        Ok(result) => answered(result),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", "The chat stopped while answering."),
    }
}

struct Refused(StatusCode, &'static str, String);

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        failure(self.0, self.1, &self.2)
    }
}

fn session_of(asked: &str) -> Result<String, Refused> {
    crate::pty::session_id(asked)
        .map(str::to_owned)
        .ok_or_else(|| Refused(StatusCode::BAD_REQUEST, "validation", "That is not a session id.".to_owned()))
}

fn text_of(body: Result<axum::Json<Value>, JsonRejection>, field: &str) -> Result<String, Refused> {
    let axum::Json(body) = body
        .map_err(|_| Refused(StatusCode::BAD_REQUEST, "validation", "The body is not the JSON expected.".to_owned()))?;
    let text = body[field].as_str().map(str::trim).unwrap_or_default();
    if text.is_empty() {
        return Err(Refused(StatusCode::BAD_REQUEST, "validation", format!("Say the {field}.")));
    }
    if text.len() > MOST_PROMPT_BYTES {
        let told = format!("A message may carry at most {MOST_PROMPT_BYTES} bytes.");
        return Err(Refused(StatusCode::PAYLOAD_TOO_LARGE, "too_large", told));
    }
    Ok(text.to_owned())
}

async fn list(State(shared): State<Shared>) -> Response {
    on_desk(&shared, |chats| chats.list()).await
}

async fn open(State(shared): State<Shared>, Path(session): Path<String>) -> Response {
    let session = match session_of(&session) {
        Ok(session) => session,
        Err(refused) => return refused.into_response(),
    };
    on_desk(&shared, move |chats| chats.open(&session)).await
}

async fn start(State(shared): State<Shared>, body: Result<axum::Json<Value>, JsonRejection>) -> Response {
    let text = match text_of(body, "text") {
        Ok(text) => text,
        Err(refused) => return refused.into_response(),
    };
    on_desk(&shared, move |chats| chats.start(&text).map(|session| json!({ "session": session }))).await
}

async fn send(
    State(shared): State<Shared>,
    Path(session): Path<String>,
    body: Result<axum::Json<Value>, JsonRejection>,
) -> Response {
    let (session, text) = match session_of(&session).and_then(|session| Ok((session, text_of(body, "text")?))) {
        Ok(asked) => asked,
        Err(refused) => return refused.into_response(),
    };
    on_desk(&shared, move |chats| chats.send(&session, &text).map(|()| json!({ "session": session }))).await
}

async fn answer(
    State(shared): State<Shared>,
    Path(session): Path<String>,
    body: Result<axum::Json<Value>, JsonRejection>,
) -> Response {
    let Ok(axum::Json(body)) = body else {
        return failure(StatusCode::BAD_REQUEST, "validation", "The body is not the JSON expected.");
    };
    let session = match session_of(&session) {
        Ok(session) => session,
        Err(refused) => return refused.into_response(),
    };
    let field = |name: &str| body[name].as_str().unwrap_or_default().to_owned();
    let (ask, decision) = (field("ask"), field("decision"));
    if ask.is_empty() || decision.is_empty() {
        return failure(StatusCode::BAD_REQUEST, "validation", "Say which ask and the decision.");
    }
    on_desk(&shared, move |chats| chats.answer(&session, &ask, &decision).map(|()| json!({}))).await
}

async fn stop(State(shared): State<Shared>, Path(session): Path<String>) -> Response {
    let session = match session_of(&session) {
        Ok(session) => session,
        Err(refused) => return refused.into_response(),
    };
    on_desk(&shared, move |chats| chats.stop(&session).map(|()| json!({}))).await
}
