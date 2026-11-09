//! The loopback server Claude Code is pointed at. Each run is admitted with
//! its own token, which names the provider its requests go to.

use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::{Arc, Mutex, PoisonError};

use crossbeam_channel::Receiver;
use serde_json::{Value, json};
use superfik_core::{GatewayAccess, ModelProvider};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::messages::MessagesRequest;
use crate::stream::{Event, message_from};

pub trait Upstream: Send + Sync {
    fn messages(&self, request: &MessagesRequest, emit: &mut dyn FnMut(Event)) -> Result<(), UpstreamError>;
}

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Failed(String),
}

impl UpstreamError {
    fn kind(&self) -> (u16, &'static str) {
        match self {
            Self::Unavailable(_) => (529, "overloaded_error"),
            Self::Invalid(_) => (400, "invalid_request_error"),
            Self::Unauthorized(_) => (401, "authentication_error"),
            Self::Failed(_) => (500, "api_error"),
        }
    }
}

#[derive(Default)]
pub struct Upstreams(HashMap<ModelProvider, Arc<dyn Upstream>>);

impl Upstreams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, provider: ModelProvider, upstream: Arc<dyn Upstream>) -> Self {
        self.0.insert(provider, upstream);
        self
    }
}

impl std::fmt::Debug for Upstreams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

type Tokens = Arc<Mutex<HashMap<String, ModelProvider>>>;

pub struct Gateway {
    base_url: String,
    tokens: Tokens,
    server: Arc<Server>,
}

impl Gateway {
    pub fn start(upstreams: Upstreams) -> io::Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(io::Error::other)?;
        let port = server.server_addr().to_ip().map(|addr| addr.port()).ok_or_else(|| io::Error::other("no port"))?;
        let server = Arc::new(server);
        let tokens = Tokens::default();
        let upstreams = Arc::new(upstreams);
        let (listening, admitted) = (server.clone(), tokens.clone());
        std::thread::Builder::new().name("superfik-gateway".into()).spawn(move || {
            for request in listening.incoming_requests() {
                let (tokens, upstreams) = (admitted.clone(), upstreams.clone());
                let spawned = std::thread::Builder::new()
                    .name("superfik-gateway-request".into())
                    .spawn(move || serve(request, &tokens, &upstreams));
                if let Err(error) = spawned {
                    tracing::warn!(%error, "gateway could not serve a request");
                }
            }
        })?;
        Ok(Self { base_url: format!("http://127.0.0.1:{port}"), tokens, server })
    }

    pub fn admit(&self, provider: ModelProvider) -> GatewayAccess {
        let token = uuid::Uuid::new_v4().simple().to_string();
        self.tokens.lock().unwrap_or_else(PoisonError::into_inner).insert(token.clone(), provider);
        GatewayAccess::new(&self.base_url, token)
    }

    pub fn revoke(&self, token: &str) {
        self.tokens.lock().unwrap_or_else(PoisonError::into_inner).remove(token);
    }
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway").field("base_url", &self.base_url).finish_non_exhaustive()
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.server.unblock();
    }
}

fn serve(mut request: Request, tokens: &Tokens, upstreams: &Upstreams) {
    let provider = token(&request).and_then(|t| tokens.lock().unwrap_or_else(PoisonError::into_inner).get(&t).copied());
    let Some(provider) = provider else {
        return error(request, 401, "authentication_error", "unknown gateway token");
    };
    if *request.method() != Method::Post {
        return error(request, 405, "invalid_request_error", "only POST is served");
    }
    let path = request.url().split('?').next().unwrap_or_default().to_owned();
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return error(request, 400, "invalid_request_error", "unreadable body");
    }
    match path.as_str() {
        "/v1/messages/count_tokens" => respond_json(request, 200, &json!({ "input_tokens": estimate_tokens(&body) })),
        "/v1/messages" => {
            let parsed = match serde_json::from_str::<MessagesRequest>(&body) {
                Ok(parsed) => parsed,
                Err(problem) => return error(request, 400, "invalid_request_error", &problem.to_string()),
            };
            let Some(upstream) = upstreams.0.get(&provider).cloned() else {
                return error(request, 404, "not_found_error", "no upstream serves this provider");
            };
            messages(request, parsed, upstream);
        }
        _ => error(request, 404, "not_found_error", "not found"),
    }
}

fn token(request: &Request) -> Option<String> {
    let header = |name: &'static str| {
        request.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_owned())
    };
    header("Authorization")
        .and_then(|value| value.strip_prefix("Bearer ").map(str::to_owned))
        .or_else(|| header("x-api-key"))
}

fn estimate_tokens(body: &str) -> usize {
    body.chars().count().div_ceil(4)
}

fn messages(request: Request, parsed: MessagesRequest, upstream: Arc<dyn Upstream>) {
    let (sender, events) = crossbeam_channel::unbounded::<Result<Event, UpstreamError>>();
    let streaming = parsed.stream;
    std::thread::spawn(move || {
        let mut emit = |event| {
            let _ = sender.send(Ok(event));
        };
        if let Err(problem) = upstream.messages(&parsed, &mut emit) {
            let _ = sender.send(Err(problem));
        }
    });
    let first = match events.recv() {
        Ok(Ok(event)) => event,
        Ok(Err(problem)) => {
            let (status, kind) = problem.kind();
            return error(request, status, kind, &problem.to_string());
        }
        Err(_) => return error(request, 500, "api_error", "the upstream gave no answer"),
    };
    if !streaming {
        let mut all = vec![first];
        for next in events.iter() {
            match next {
                Ok(event) => all.push(event),
                Err(problem) => {
                    let (status, kind) = problem.kind();
                    return error(request, status, kind, &problem.to_string());
                }
            }
        }
        return respond_json(request, 200, &message_from(&all));
    }
    let reader = SseReader { first: Some(first), events, buffer: Vec::new(), at: 0 };
    let response = Response::new(
        200.into(),
        header("Content-Type", "text/event-stream").into_iter().collect(),
        reader,
        None,
        None,
    );
    if let Err(error) = request.respond(response) {
        tracing::debug!(%error, "gateway stream closed early");
    }
}

struct SseReader {
    first: Option<Event>,
    events: Receiver<Result<Event, UpstreamError>>,
    buffer: Vec<u8>,
    at: usize,
}

impl Read for SseReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.at == self.buffer.len() {
            let next = match self.first.take() {
                Some(event) => event,
                None => match self.events.recv() {
                    Ok(Ok(event)) => event,
                    Ok(Err(problem)) => {
                        let (_, kind) = problem.kind();
                        Event { name: "error", data: error_body(kind, &problem.to_string()) }
                    }
                    Err(_) => return Ok(0),
                },
            };
            self.buffer = next.to_sse().into_bytes();
            self.at = 0;
        }
        let n = out.len().min(self.buffer.len() - self.at);
        out[..n].copy_from_slice(&self.buffer[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

fn error_body(kind: &str, message: &str) -> Value {
    json!({ "type": "error", "error": { "type": kind, "message": message } })
}

fn error(request: Request, status: u16, kind: &str, message: &str) {
    respond_json(request, status, &error_body(kind, message));
}

fn respond_json(request: Request, status: u16, body: &Value) {
    let mut response = Response::from_string(body.to_string()).with_status_code(status);
    if let Some(content_type) = header("Content-Type", "application/json") {
        response.add_header(content_type);
    }
    if let Err(error) = request.respond(response) {
        tracing::debug!(%error, "gateway could not answer");
    }
}

fn header(name: &str, value: &str) -> Option<Header> {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
}
