use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use serde_json::{Value, json};
use tower_http::services::{ServeDir, ServeFile};

use crate::{Assets, Backend};

#[derive(Debug, Clone)]
pub struct Settings {
    pub token: String,
    pub assets: Assets,
}

#[derive(Clone)]
pub struct Shared {
    pub token: Arc<str>,
    pub backend: Arc<dyn Backend>,
}

pub fn router(backend: Arc<dyn Backend>, settings: Settings) -> Router {
    let shared = Shared { token: settings.token.into(), backend };
    let api = Router::new()
        .route("/api/health", get(|| async { axum::Json(json!({ "status": "ok" })) }))
        .route("/api/instance", get(instance))
        .route("/api/ws", get(crate::realtime::socket))
        .merge(crate::organization::routes())
        .route("/api/{*rest}", any(|| async { failure(StatusCode::NOT_FOUND, "not_found", "No such endpoint.") }));
    let app = match settings.assets {
        Assets::Folder(dist) => {
            api.fallback_service(ServeDir::new(&dist).fallback(ServeFile::new(dist.join("index.html"))))
        }
        Assets::Bundle(bundle) => api.fallback(move |uri: Uri| async move { crate::assets::serve(bundle, &uri) }),
        Assets::Missing => api.fallback(|| async { (StatusCode::SERVICE_UNAVAILABLE, Html(crate::assets::MISSING)) }),
    };
    app.layer(axum::middleware::from_fn_with_state(shared.clone(), crate::access::guard)).with_state(shared)
}

async fn instance(State(shared): State<Shared>) -> Response {
    feature(&shared, "instance", json!({})).await
}

async fn feature(shared: &Shared, name: &'static str, input: Value) -> Response {
    let backend = shared.backend.clone();
    match tokio::task::spawn_blocking(move || backend.feature(name, input)).await {
        Ok(Ok(output)) => axum::Json(output).into_response(),
        Ok(Err(message)) => failure(StatusCode::INTERNAL_SERVER_ERROR, "backend", &message),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "backend", "The backend stopped answering."),
    }
}

pub fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (status, axum::Json(json!({ "error": { "code": code, "message": message } }))).into_response()
}
