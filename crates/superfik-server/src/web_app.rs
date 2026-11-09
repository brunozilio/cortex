//! The web app (ADR 0010, `superfik-web`), served by the server itself:
//! the page and its wasm come from the same origin as `/ws`, so the
//! session cookie reaches the socket and no other site is involved.
//! `SUPERFIK_WEB` names the folder the bundle was built into; without it
//! the server serves no page.

use std::path::Path;

use axum::Router;
use axum::http::{HeaderValue, header};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

pub fn with_web_app(routes: Router, dir: Option<&Path>) -> Router {
    match dir {
        Some(dir) => {
            let bundle = ServeDir::new(dir).append_index_html_on_directories(true);
            let no_cache = HeaderValue::from_static("no-cache");
            let revalidated = SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, no_cache);
            let bundle = crate::headers::hardened(Router::new().fallback_service(bundle));
            routes.fallback_service(ServiceBuilder::new().layer(revalidated).service(bundle))
        }
        None => routes,
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::get;
    use tower::ServiceExt;

    use super::*;

    async fn get_path(routes: Router, path: &str) -> (StatusCode, Option<String>) {
        let request = Request::builder().uri(path).body(Body::empty()).expect("request");
        let response = routes.oneshot(request).await.expect("response");
        let kind = response.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_owned);
        (response.status(), kind)
    }

    #[tokio::test]
    async fn serves_the_page_and_its_wasm_beside_the_routes() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("index.html"), "<canvas id=superfik>").expect("page");
        std::fs::write(dir.path().join("superfik_web_bg.wasm"), b"\0asm").expect("wasm");
        let routes = || with_web_app(Router::new().route("/ws", get(|| async { "socket" })), Some(dir.path()));
        assert_eq!(get_path(routes(), "/").await.0, StatusCode::OK);
        let (status, kind) = get_path(routes(), "/superfik_web_bg.wasm").await;
        assert_eq!((status, kind.as_deref()), (StatusCode::OK, Some("application/wasm")), "browsers stream it");
        assert_eq!(get_path(routes(), "/ws").await.0, StatusCode::OK, "the server's own routes win");
        assert_eq!(get_path(routes(), "/nope.js").await.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_new_build_is_never_hidden_by_the_browsers_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("superfik_web.js"), "export default 1").expect("js");
        let routes = with_web_app(Router::new(), Some(dir.path()));
        let request = Request::builder().uri("/superfik_web.js").body(Body::empty()).expect("request");
        let response = routes.oneshot(request).await.expect("response");
        let cache = response.headers().get(header::CACHE_CONTROL).and_then(|v| v.to_str().ok());
        assert_eq!(cache, Some("no-cache"), "the page and its wasm must match");
        let framed = response.headers().get(header::X_FRAME_OPTIONS).and_then(|v| v.to_str().ok());
        assert_eq!(framed, Some("DENY"), "the app is never framed by another site");
    }

    #[tokio::test]
    async fn without_a_bundle_there_is_no_page() {
        assert_eq!(get_path(with_web_app(Router::new(), None), "/").await.0, StatusCode::NOT_FOUND);
    }
}
