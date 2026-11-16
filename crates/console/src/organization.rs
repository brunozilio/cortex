use std::collections::HashMap;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::on;
use serde_json::{Map, Value, json};

use crate::realtime::ORG;
use crate::routes::{Shared, failure};

const BODY_LIMIT: usize = 4 * 1024 * 1024;

struct Operation {
    method: Method,
    path: &'static str,
    feature: &'static str,
    op: &'static str,
}

const fn operation(method: Method, path: &'static str, feature: &'static str, op: &'static str) -> Operation {
    Operation { method, path, feature, op }
}

fn operations() -> Vec<Operation> {
    let mut all = vec![
        operation(Method::GET, "/me/sidebar", "sidebar", "items"),
        operation(Method::PATCH, "/me/sidebar", "sidebar", "set_item"),
        operation(Method::PUT, "/me/sidebar", "sidebar", "set_items"),
        operation(Method::GET, "/me/sidebar/order", "sidebar", "order"),
        operation(Method::PUT, "/me/sidebar/order", "sidebar", "set_order"),
        operation(Method::GET, "/me/sidebar/tools", "sidebar", "tools"),
        operation(Method::PUT, "/me/sidebar/tools", "sidebar", "set_tools"),
        operation(Method::GET, "/me/sidebar/view", "sidebar", "view"),
        operation(Method::PUT, "/me/sidebar/view", "sidebar", "set_view"),
    ];
    all.extend(profile_operations());
    all.extend(organization_operations());
    all
}

fn profile_operations() -> Vec<Operation> {
    vec![
        operation(Method::GET, "/me/prefs", "profile", "prefs"),
        operation(Method::PATCH, "/me", "profile", "update_me"),
        operation(Method::PUT, "/me/start", "profile", "set_start"),
        operation(Method::PUT, "/me/activity", "profile", "set_activity"),
        operation(Method::PUT, "/me/agent-style", "profile", "set_agent_style"),
        operation(Method::PUT, "/me/chat", "profile", "set_chat"),
        operation(Method::GET, "/me/issues-view", "profile", "issues_view"),
        operation(Method::PUT, "/me/issues-view", "profile", "set_issues_view"),
        operation(Method::PUT, "/me/avatar", "profile", "set_avatar"),
        operation(Method::DELETE, "/me/avatar", "profile", "remove_avatar"),
    ]
}

fn organization_operations() -> Vec<Operation> {
    vec![
        operation(Method::GET, "/settings", "organization", "settings"),
        operation(Method::PATCH, "/settings", "organization", "update_settings"),
        operation(Method::PUT, "/approvals-level", "organization", "set_approvals_level"),
        operation(Method::GET, "/overview", "organization", "overview"),
        operation(Method::GET, "/limits", "organization", "limits"),
        operation(Method::GET, "/logo", "organization", "logo"),
        operation(Method::PUT, "/logo", "organization", "set_logo"),
        operation(Method::DELETE, "/logo", "organization", "remove_logo"),
        operation(Method::GET, "/usage/report", "usage", "report"),
        operation(Method::GET, "/usage/plans", "usage", "plans"),
    ]
}

type Asked = Query<HashMap<String, String>>;

pub fn routes() -> Router<Shared> {
    let router = operations().into_iter().fold(Router::new(), |router: Router<Shared>, operation| {
        let path = format!("/api/app/organizations/{{org}}{}", operation.path);
        let filter = axum::routing::MethodFilter::try_from(operation.method.clone());
        let Ok(filter) = filter else { return router };
        let (feature, op, writes) = (operation.feature, operation.op, operation.method != Method::GET);
        router.route(
            &path,
            on(filter, move |State(shared): State<Shared>, Path(org): Path<String>, Query(asked): Asked, body: Bytes| {
                async move { answer(&shared, &org, (feature, op, writes), asked, &body).await }
            }),
        )
    });
    router.layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn answer(
    shared: &Shared,
    org: &str,
    call: (&'static str, &'static str, bool),
    asked: HashMap<String, String>,
    body: &[u8],
) -> Response {
    let (feature, op, writes) = call;
    if org != ORG {
        return failure(StatusCode::NOT_FOUND, "not_found", "No such organization.");
    }
    let mut input: Map<String, Value> = asked.into_iter().map(|(key, value)| (key, json!(value))).collect();
    if !body.is_empty() {
        match serde_json::from_slice::<Value>(body) {
            Ok(Value::Object(fields)) => input.extend(fields),
            _ => return failure(StatusCode::BAD_REQUEST, "validation", "The body must be a JSON object."),
        }
    }
    input.insert("op".into(), json!(op));
    let backend = shared.backend.clone();
    match tokio::task::spawn_blocking(move || backend.feature(feature, Value::Object(input))).await {
        Ok(Ok(Value::Null)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Ok(output)) => axum::Json(output).into_response(),
        Ok(Err(message)) if writes => failure(StatusCode::BAD_REQUEST, "validation", &message),
        Ok(Err(message)) => failure(StatusCode::INTERNAL_SERVER_ERROR, "backend", &message),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "backend", "The backend stopped answering."),
    }
}
