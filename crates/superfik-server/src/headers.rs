//! What every answer of the server says to browsers about itself: never
//! framed, never sniffed into another type, no referrer sent on. Pages set
//! their own fuller `Content-Security-Policy` ([`PAGE_POLICY`]); every
//! other answer gets one forbidding frames.

use axum::Router;
use axum::http::{HeaderName, HeaderValue, header};
use tower_http::set_header::SetResponseHeaderLayer;

pub const PAGE_POLICY: &str =
    "default-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; object-src 'none'";

pub fn hardened<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    let set = |name: HeaderName, value: &'static str| {
        SetResponseHeaderLayer::if_not_present(name, HeaderValue::from_static(value))
    };
    router
        .layer(set(header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .layer(set(header::X_FRAME_OPTIONS, "DENY"))
        .layer(set(header::REFERRER_POLICY, "no-referrer"))
        .layer(set(header::CONTENT_SECURITY_POLICY, "frame-ancestors 'none'"))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn every_answer_is_never_framed_sniffed_or_referred() {
        let routes = hardened(Router::new().route("/x", get(|| async { "x" })));
        let request = Request::get("/x").body(Body::empty()).expect("request");
        let response = routes.oneshot(request).await.expect("response");
        let header = |name: HeaderName| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
        assert_eq!(header(header::X_CONTENT_TYPE_OPTIONS).as_deref(), Some("nosniff"));
        assert_eq!(header(header::X_FRAME_OPTIONS).as_deref(), Some("DENY"));
        assert_eq!(header(header::REFERRER_POLICY).as_deref(), Some("no-referrer"));
        assert_eq!(header(header::CONTENT_SECURITY_POLICY).as_deref(), Some("frame-ancestors 'none'"));
    }
}
