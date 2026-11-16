use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::routes::{Shared, failure};

pub const COOKIE: &str = "console_token";
const LINK: &str = "token";

pub async fn guard(State(shared): State<Shared>, request: Request, next: Next) -> Response {
    if let Some(offered) = link_token(request.uri()) {
        return enter(&shared.token, &offered, request.uri());
    }
    if foreign_origin(request.headers()) {
        return failure(StatusCode::FORBIDDEN, "forbidden", "This request came from another site.");
    }
    let api = request.uri().path().starts_with("/api/");
    if api && !cookie_token(request.headers()).is_some_and(|token| same(token, &shared.token)) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Open the console from the link the terminal printed.",
        );
    }
    next.run(request).await
}

fn enter(token: &str, offered: &str, uri: &Uri) -> Response {
    if !same(offered, token) {
        return failure(StatusCode::UNAUTHORIZED, "unauthorized", "This console link is not valid any more.");
    }
    let cookie = format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict");
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    let headers = response.headers_mut();
    if let (Ok(location), Ok(cookie)) = (HeaderValue::from_str(&without_token(uri)), HeaderValue::from_str(&cookie)) {
        headers.insert(header::LOCATION, location);
        headers.insert(header::SET_COOKIE, cookie);
        return response;
    }
    StatusCode::BAD_REQUEST.into_response()
}

fn pairs(uri: &Uri) -> impl Iterator<Item = (&str, &str)> {
    uri.query().into_iter().flat_map(|query| query.split('&')).map(|pair| pair.split_once('=').unwrap_or((pair, "")))
}

fn link_token(uri: &Uri) -> Option<String> {
    pairs(uri).find(|(key, _)| *key == LINK).map(|(_, value)| value.to_owned())
}

fn without_token(uri: &Uri) -> String {
    let kept: Vec<String> = pairs(uri)
        .filter(|(key, _)| *key != LINK)
        .map(|(key, value)| if value.is_empty() { key.to_owned() } else { format!("{key}={value}") })
        .collect();
    if kept.is_empty() { uri.path().to_owned() } else { format!("{}?{}", uri.path(), kept.join("&")) }
}

fn cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value)
}

fn foreign_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|origin| origin.to_str().ok()) else { return false };
    let host = headers.get(header::HOST).and_then(|host| host.to_str().ok()).unwrap_or_default();
    origin != format!("http://{host}")
}

fn same(left: &str, right: &str) -> bool {
    left.len() == right.len() && left.bytes().zip(right.bytes()).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}
