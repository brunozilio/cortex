#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use console::{Assets, Backend, Settings};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const COOKIE: &str = "console_token=t";

#[derive(Default)]
struct Recording {
    calls: Mutex<Vec<(String, Value)>>,
}

impl Backend for Recording {
    fn feature(&self, name: &str, input: Value) -> Result<Value, String> {
        self.calls.lock().expect("calls").push((name.to_owned(), input.clone()));
        if input["refuse"] == true {
            return Err("that choice is not allowed".into());
        }
        if input["op"].as_str().is_some_and(|op| op.starts_with("remove_")) {
            return Ok(Value::Null);
        }
        Ok(json!({ "answered": input["op"] }))
    }
}

async fn call(backend: &Arc<Recording>, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let app = console::router(backend.clone(), Settings { token: "t".into(), assets: Assets::Missing });
    let mut request = Request::builder().method(method).uri(path).header(header::COOKIE, COOKIE);
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let body = body.map_or_else(Body::empty, |body| Body::from(body.to_string()));
    let response = app.oneshot(request.body(body).expect("request")).await.expect("response");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn last(backend: &Recording) -> (String, Value) {
    backend.calls.lock().expect("calls").last().cloned().expect("a call")
}

const ORG: &str = "/api/app/organizations/local";

#[tokio::test]
async fn reading_the_sidebar_asks_the_sidebar_feature() {
    let backend = Arc::new(Recording::default());
    for (path, op) in [
        ("/me/sidebar", "items"),
        ("/me/sidebar/order", "order"),
        ("/me/sidebar/tools", "tools"),
        ("/me/sidebar/view", "view"),
    ] {
        let (status, body) = call(&backend, "GET", &format!("{ORG}{path}"), None).await;
        assert_eq!((status, body), (StatusCode::OK, json!({ "answered": op })), "{path}");
        assert_eq!(last(&backend), ("sidebar".into(), json!({ "op": op })));
    }
}

#[tokio::test]
async fn a_change_sends_its_body_with_the_operation() {
    let backend = Arc::new(Recording::default());
    let (status, _) =
        call(&backend, "PATCH", &format!("{ORG}/me/sidebar"), Some(json!({ "item": "wiki", "place": "more" }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(last(&backend), ("sidebar".into(), json!({ "op": "set_item", "item": "wiki", "place": "more" })));

    call(&backend, "PUT", &format!("{ORG}/me/sidebar/order"), Some(json!({ "order": null }))).await;
    assert_eq!(last(&backend), ("sidebar".into(), json!({ "op": "set_order", "order": null })));
}

#[tokio::test]
async fn a_refused_change_is_a_validation_error() {
    let backend = Arc::new(Recording::default());
    let (status, body) =
        call(&backend, "PUT", &format!("{ORG}/me/sidebar/view"), Some(json!({ "refuse": true }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation");
    assert_eq!(body["error"]["message"], "that choice is not allowed");
}

#[tokio::test]
async fn another_organization_is_not_found() {
    let backend = Arc::new(Recording::default());
    let (status, _) = call(&backend, "GET", "/api/app/organizations/other/me/sidebar", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(backend.calls.lock().expect("calls").is_empty());
}

#[tokio::test]
async fn profile_settings_and_usage_routes_reach_their_features() {
    let backend = Arc::new(Recording::default());
    for (method, path, feature, op) in [
        ("GET", "/me/prefs", "profile", "prefs"),
        ("PATCH", "/me", "profile", "update_me"),
        ("PUT", "/me/start", "profile", "set_start"),
        ("PUT", "/me/activity", "profile", "set_activity"),
        ("PUT", "/me/agent-style", "profile", "set_agent_style"),
        ("PUT", "/me/chat", "profile", "set_chat"),
        ("GET", "/me/issues-view", "profile", "issues_view"),
        ("PUT", "/me/issues-view", "profile", "set_issues_view"),
        ("PUT", "/me/avatar", "profile", "set_avatar"),
        ("GET", "/settings", "organization", "settings"),
        ("PATCH", "/settings", "organization", "update_settings"),
        ("PUT", "/approvals-level", "organization", "set_approvals_level"),
        ("GET", "/overview", "organization", "overview"),
        ("GET", "/limits", "organization", "limits"),
        ("GET", "/logo", "organization", "logo"),
        ("PUT", "/logo", "organization", "set_logo"),
        ("GET", "/usage/report", "usage", "report"),
        ("GET", "/usage/plans", "usage", "plans"),
    ] {
        let body = (method != "GET").then(|| json!({}));
        let (status, answer) = call(&backend, method, &format!("{ORG}{path}"), body).await;
        assert_eq!((status, answer), (StatusCode::OK, json!({ "answered": op })), "{method} {path}");
        assert_eq!(last(&backend), (feature.into(), json!({ "op": op })), "{method} {path}");
    }
}

#[tokio::test]
async fn a_removal_answers_no_content() {
    let backend = Arc::new(Recording::default());
    for (path, op) in [("/me/avatar", "remove_avatar"), ("/logo", "remove_logo")] {
        let (status, body) = call(&backend, "DELETE", &format!("{ORG}{path}"), None).await;
        assert_eq!((status, body), (StatusCode::NO_CONTENT, Value::Null), "{path}");
        assert_eq!(last(&backend).1, json!({ "op": op }));
    }
}

#[tokio::test]
async fn the_query_reaches_the_feature_with_the_operation() {
    let backend = Arc::new(Recording::default());
    let (status, _) = call(&backend, "GET", &format!("{ORG}/usage/report?since=2026-10-01T00:00:00Z"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(last(&backend), ("usage".into(), json!({ "op": "report", "since": "2026-10-01T00:00:00Z" })));
}

#[tokio::test]
async fn a_photo_of_two_megabytes_fits_in_the_body() {
    let backend = Arc::new(Recording::default());
    let image = "A".repeat(2_900_000);
    let body = json!({ "imageBase64": image, "contentType": "image/png" });
    let (status, _) = call(&backend, "PUT", &format!("{ORG}/me/avatar"), Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(last(&backend).1["imageBase64"].as_str().map(str::len), Some(2_900_000));
}
