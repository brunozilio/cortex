use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};

use crate::{Assets, Launch, Live};

#[derive(Debug, Clone)]
pub struct Settings {
    pub token: String,
    pub port: u16,
    pub assets: Assets,
    pub terminal: Launch,
}

#[derive(Debug, Clone)]
pub struct Shared {
    pub token: Arc<str>,
    pub port: u16,
    pub live: Live,
    pub terminal: Launch,
    pub terminals: Arc<tokio::sync::Semaphore>,
    pub subscribers: Arc<tokio::sync::Semaphore>,
}

pub fn router(settings: Settings, live: Live) -> Router {
    let shared = Shared {
        token: settings.token.into(),
        port: settings.port,
        live,
        terminal: settings.terminal,
        terminals: Arc::new(tokio::sync::Semaphore::new(crate::terminal::MOST_AT_ONCE)),
        subscribers: Arc::new(tokio::sync::Semaphore::new(crate::live::MOST_SUBSCRIBERS)),
    };
    let api = Router::new()
        .route("/api/health", get(|| async { axum::Json(json!({ "status": "ok" })) }))
        .route("/api/instance", get(|| async { axum::Json(json!({ "name": brand::NAME })) }))
        .merge(crate::live::routes())
        .merge(crate::terminal::routes())
        .route("/api/{*rest}", any(|| async { failure(StatusCode::NOT_FOUND, "not_found", "No such endpoint.") }));
    let app = match settings.assets {
        Assets::Folder(dist) => {
            api.fallback_service(ServeDir::new(&dist).fallback(ServeFile::new(dist.join("index.html"))))
        }
        Assets::Bundle(bundle) => api.fallback(move |uri: Uri| async move { crate::assets::serve(bundle, &uri) }),
        Assets::Missing => api.fallback(|| async { (StatusCode::SERVICE_UNAVAILABLE, Html(crate::assets::MISSING)) }),
    };
    app.layer(DefaultBodyLimit::max(crate::live::MOST_BODY_BYTES))
        .layer(axum::middleware::from_fn_with_state(shared.clone(), crate::access::guard))
        .with_state(shared)
}

pub fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (status, axum::Json(json!({ "error": { "code": code, "message": message } }))).into_response()
}
