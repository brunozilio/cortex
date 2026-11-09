//! Routes for an organization's tooling, not its members' apps: the audit
//! log as JSON lines (`GET /admin/audit?since=<unix>[&skip=<n>][&limit=<n>]`),
//! for a SIEM to collect, a page at a time: a full page names where the
//! next starts in its `x-audit-next` header. Owners and admins only, with
//! their bearer token.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use superfik_core::Principal;

use crate::audit::{AuditLog, next_page};
use crate::{Authenticator, Organization};

#[derive(Clone)]
struct Admin {
    org: Arc<Organization>,
    audit: Arc<dyn AuditLog>,
}

pub fn routes(org: Arc<Organization>, audit: Arc<dyn AuditLog>) -> Router {
    Router::new().route("/admin/audit", axum::routing::get(audit_log)).with_state(Admin { org, audit })
}

const PAGE: usize = 1_000;
const MOST: usize = 10_000;

pub const NEXT: &str = "x-audit-next";

#[derive(Deserialize)]
struct Since {
    #[serde(default)]
    since: u64,
    #[serde(default)]
    skip: usize,
    limit: Option<usize>,
}

async fn audit_log(
    State(admin): State<Admin>,
    headers: HeaderMap,
    Query(Since { since, skip, limit }): Query<Since>,
) -> Response {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let token = crate::session::token(&headers).or(bearer);
    let Some(Principal::Member(member)) = token.and_then(|token| admin.org.authenticate(token)) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !admin.org.directory.role(&member).is_some_and(superfik_core::access::administers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let limit = limit.unwrap_or(PAGE).clamp(1, MOST);
    let audit = Arc::clone(&admin.audit);
    let read = tokio::task::spawn_blocking(move || {
        let page = audit.page(since, skip, limit);
        let mut body = String::new();
        for entry in &page {
            if let Ok(line) = serde_json::to_string(entry) {
                body.push_str(&line);
                body.push('\n');
            }
        }
        (body, next_page(since, skip, &page, limit))
    })
    .await;
    let Ok((body, next)) = read else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    let mut response = ([(header::CONTENT_TYPE, "application/x-ndjson")], body).into_response();
    if let Some((since, skip)) = next
        && let Ok(value) = HeaderValue::from_str(&format!("since={since}&skip={skip}"))
    {
        response.headers_mut().insert(NEXT, value);
    }
    response
}
