use std::sync::Arc;

use axum::Router;
use axum::http::{StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};

use crate::{Assets, Live};

#[derive(Debug, Clone)]
pub struct Settings {
    pub token: String,
    pub assets: Assets,
}

#[derive(Debug, Clone)]
pub struct Shared {
    pub token: Arc<str>,
    pub live: Live,
}

pub fn router(settings: Settings, live: Live) -> Router {
    let shared = Shared { token: settings.token.into(), live };
    let api = Router::new()
        .route("/api/health", get(|| async { axum::Json(json!({ "status": "ok" })) }))
        .merge(crate::live::routes())
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

pub fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (status, axum::Json(json!({ "error": { "code": code, "message": message } }))).into_response()
}
