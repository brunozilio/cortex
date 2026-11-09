//! Signing in to the server over HTTP, for the app and for browsers:
//!
//! - `POST /auth/first-access {name, code, password}`: the first access
//!   with the code an admin handed over; the password becomes the user's.
//! - `POST /auth/sign-in {name, password}`: every later one.
//!
//! Both answer `{token}` and set the session cookie (`session`), so the
//! desktop keeps the token and a browser the cookie. `GET /sign-in` is the
//! page a browser signs in on. A request naming an `Origin` must come from
//! one the server allows, like every other.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use superfik_identity::IdentityError;

use crate::Organization;
use crate::session::{COOKIE, Origins};

#[derive(Clone)]
struct SignIns {
    org: Arc<Organization>,
    origins: Origins,
}

pub fn routes(org: Arc<Organization>, origins: Origins) -> Router {
    let routes = Router::new()
        .route("/auth/first-access", post(first_access))
        .route("/auth/sign-in", post(sign_in))
        .route("/sign-in", get(page))
        .route("/sign-in.js", get(script))
        .route("/sign-in.css", get(style))
        .with_state(SignIns { org, origins });
    crate::headers::hardened(routes)
}

#[derive(Deserialize)]
struct FirstAccess {
    name: String,
    code: String,
    password: String,
}

#[derive(Deserialize)]
struct Credentials {
    name: String,
    password: String,
}

async fn first_access(State(s): State<SignIns>, headers: HeaderMap, Json(body): Json<FirstAccess>) -> Response {
    if !s.origins.admit(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let org = Arc::clone(&s.org);
    let outcome = tokio::task::spawn_blocking(move || {
        org.directory.first_access(&body.name, &body.code, &body.password, org.now())
    })
    .await;
    signed_in(outcome)
}

async fn sign_in(State(s): State<SignIns>, headers: HeaderMap, Json(body): Json<Credentials>) -> Response {
    if !s.origins.admit(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let org = Arc::clone(&s.org);
    let outcome =
        tokio::task::spawn_blocking(move || org.directory.sign_in(&body.name, &body.password, org.now())).await;
    signed_in(outcome)
}

fn signed_in(outcome: Result<Result<String, IdentityError>, tokio::task::JoinError>) -> Response {
    let token = match outcome {
        Ok(Ok(token)) => token,
        Ok(Err(error)) => {
            let status = match error {
                IdentityError::WrongCode | IdentityError::WrongCredentials => StatusCode::UNAUTHORIZED,
                IdentityError::TooManyAttempts => StatusCode::TOO_MANY_REQUESTS,
                IdentityError::WeakPassword | IdentityError::BadName => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return (status, Json(serde_json::json!({ "error": error.to_string() }))).into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let cookie = format!("{COOKIE}={token}; HttpOnly; Secure; SameSite=Strict; Path=/");
    ([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "token": token }))).into_response()
}

async fn page() -> impl IntoResponse {
    ([(header::CONTENT_SECURITY_POLICY, crate::headers::PAGE_POLICY)], Html(include_str!("sign_in_page.html")))
}

async fn script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8")], include_str!("sign_in.js"))
}

async fn style() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], include_str!("sign_in.css"))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{HeaderMap, Request, StatusCode, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    async fn get(path: &str) -> (StatusCode, HeaderMap, String) {
        let org = Arc::new(crate::testing::organization(Arc::new(|| 1)));
        let request = Request::get(path).body(Body::empty()).expect("request");
        let response = routes(org, Origins::default()).oneshot(request).await.expect("response");
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.expect("body").to_bytes();
        (parts.status, parts.headers, String::from_utf8_lossy(&bytes).into_owned())
    }

    #[tokio::test]
    async fn the_page_runs_only_its_own_files_and_is_never_framed() {
        let (status, headers, page) = get("/sign-in").await;
        assert_eq!(status, StatusCode::OK);
        let policy = headers.get(header::CONTENT_SECURITY_POLICY).and_then(|v| v.to_str().ok()).expect("a policy");
        for directive in ["default-src 'self'", "frame-ancestors 'none'", "form-action 'self'", "base-uri 'none'"] {
            assert!(policy.contains(directive), "{directive} missing: {policy}");
        }
        assert!(!policy.contains("unsafe-inline"), "{policy}");
        assert_eq!(headers.get(header::X_FRAME_OPTIONS).map(|v| v.as_bytes()), Some(&b"DENY"[..]));
        assert_eq!(headers.get(header::REFERRER_POLICY).map(|v| v.as_bytes()), Some(&b"no-referrer"[..]));
        assert_eq!(headers.get(header::X_CONTENT_TYPE_OPTIONS).map(|v| v.as_bytes()), Some(&b"nosniff"[..]));
        assert!(!page.contains("<script>") && !page.contains("<style>"), "nothing inline for the policy to allow");
    }

    #[tokio::test]
    async fn the_pages_script_and_style_are_served_beside_it() {
        let (status, headers, script) = get("/sign-in.js").await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get(header::CONTENT_TYPE).is_some_and(|v| v.as_bytes().starts_with(b"text/javascript")));
        assert!(script.contains("/auth/sign-in"));
        let (status, headers, _) = get("/sign-in.css").await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get(header::CONTENT_TYPE).is_some_and(|v| v.as_bytes().starts_with(b"text/css")));
    }
}
