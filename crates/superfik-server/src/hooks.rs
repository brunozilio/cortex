//! Webhooks on the network (ADR 0012).
//!
//! - Routines: `POST /hooks/<id>` with the routine's token as a bearer
//!   starts the routine with the body. The answer says only whether it was
//!   taken: an unknown address and a wrong token are the same 404, a body
//!   over the cap is 413, too many calls 429.
//! - Channel accounts on official APIs: `GET` and `POST
//!   /hooks/channels/<connection>` go to the account's link, which answers
//!   the provider's handshake (Meta's verify token, X's CRC, LinkedIn's
//!   challenge) or checks its signature and takes the events. An unknown
//!   account and a bad signature are the same 404.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use superfik_backend::{HookAnswer, HookMethod, HookRequest, WebhookRefusal};
use superfik_core::ConnectionId;
use superfik_core::channel::MAX_CHANNEL_HOOK_BYTES;
use superfik_core::routine::{MAX_WEBHOOK_BYTES, WebhookId};

use crate::Server;

pub fn routes(server: Server) -> Router {
    let routines = Router::new().route("/hooks/{hook}", post(call)).layer(DefaultBodyLimit::max(MAX_WEBHOOK_BYTES));
    let channels = Router::new()
        .route("/hooks/channels/{connection}", get(channel_get).post(channel_post))
        .layer(DefaultBodyLimit::max(MAX_CHANNEL_HOOK_BYTES));
    routines.merge(channels).with_state(server)
}

async fn call(State(server): State<Server>, Path(hook): Path<String>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let Ok(hook) = uuid::Uuid::parse_str(&hook).map(WebhookId::from_uuid) else { return StatusCode::NOT_FOUND };
    let token = bearer(&headers);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX));
    let taken =
        tokio::task::spawn_blocking(move || server.receive_webhook(hook, token.as_deref(), &body, now_ms)).await;
    match taken {
        Ok(Ok(())) => StatusCode::ACCEPTED,
        Ok(Err(WebhookRefusal::Unknown)) => StatusCode::NOT_FOUND,
        Ok(Err(WebhookRefusal::TooLarge)) => StatusCode::PAYLOAD_TOO_LARGE,
        Ok(Err(WebhookRefusal::TooMany)) => StatusCode::TOO_MANY_REQUESTS,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

type Pairs = Query<Vec<(String, String)>>;

async fn channel_get(
    State(server): State<Server>,
    Path(connection): Path<String>,
    Query(query): Pairs,
    headers: HeaderMap,
) -> Response {
    let request = channel_request(HookMethod::Get, query, &headers, Vec::new());
    channel(server, &connection, request).await
}

async fn channel_post(
    State(server): State<Server>,
    Path(connection): Path<String>,
    Query(query): Pairs,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = channel_request(HookMethod::Post, query, &headers, body.to_vec());
    channel(server, &connection, request).await
}

fn channel_request(
    method: HookMethod,
    query: Vec<(String, String)>,
    headers: &HeaderMap,
    body: Vec<u8>,
) -> HookRequest {
    let headers = headers
        .iter()
        .filter_map(|(name, value)| Some((name.as_str().to_ascii_lowercase(), value.to_str().ok()?.to_owned())))
        .collect();
    HookRequest { method, query: query.into_iter().collect(), headers, body }
}

async fn channel(server: Server, connection: &str, request: HookRequest) -> Response {
    let Ok(connection) = uuid::Uuid::parse_str(connection).map(ConnectionId::from_uuid) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let answer = tokio::task::spawn_blocking(move || server.receive_channel_hook(connection, &request)).await;
    match answer {
        Ok(HookAnswer::Accepted) => StatusCode::OK.into_response(),
        Ok(HookAnswer::Text(text)) => ([(header::CONTENT_TYPE, "text/plain")], text).into_response(),
        Ok(HookAnswer::Json(json)) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
        Ok(HookAnswer::Refused) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").map(|token| token.trim().to_owned())
}
