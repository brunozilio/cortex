//! The audit log leaves the server only for owners and admins, as JSON
//! lines from a given time, a page at a time.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use superfik_core::Principal;
use superfik_core::access::OrgRole;
use superfik_protocol::ClientCommand;
use superfik_server::audit::{AuditLog, Entry, MemoryAudit, Outcome};
use superfik_server::testing::{self, admit};
use superfik_server::{Organization, admin};
use tower::ServiceExt;

const NOW: u64 = 1_800_000_000;

struct Answer {
    status: StatusCode,
    body: String,
    next: Option<String>,
}

impl Answer {
    fn times(&self) -> Vec<u64> {
        self.body.lines().map(|line| serde_json::from_str::<Entry>(line).expect("entry").at_unix).collect()
    }
}

async fn get(org: &Arc<Organization>, audit: &Arc<MemoryAudit>, path: &str, token: Option<&str>) -> Answer {
    let mut request = Request::get(path);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let routes = admin::routes(Arc::clone(org), audit.clone());
    let response = routes.oneshot(request.body(Body::empty()).expect("request")).await.expect("response");
    let status = response.status();
    let next = response.headers().get(admin::NEXT).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    Answer { status, body: String::from_utf8_lossy(&bytes).into_owned(), next }
}

fn logged(by: &superfik_core::Member, times: &[u64]) -> Arc<MemoryAudit> {
    let audit = Arc::new(MemoryAudit::default());
    for at in times {
        audit.record(&Entry::of(&Principal::Member(*by), &ClientCommand::ListModels, *at, Outcome::Allowed));
    }
    audit
}

#[tokio::test]
async fn owners_export_the_log_from_a_time_and_members_cannot() {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let (olga, owner) = admit(&organization, "Olga", OrgRole::Owner);
    let (_, member) = admit(&organization, "Ana", OrgRole::Member);
    let audit = logged(&olga, &[10, 20, 30]);
    assert_eq!(get(&organization, &audit, "/admin/audit", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&organization, &audit, "/admin/audit", Some(&member)).await.status, StatusCode::FORBIDDEN);
    let answer = get(&organization, &audit, "/admin/audit?since=20", Some(&owner)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.times(), [20, 30]);
    assert_eq!(answer.next, None, "nothing more");
}

#[tokio::test]
async fn a_long_log_comes_a_page_at_a_time() {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let (olga, owner) = admit(&organization, "Olga", OrgRole::Owner);
    let audit = logged(&olga, &[10, 20, 20, 30]);
    let first = get(&organization, &audit, "/admin/audit?since=0&limit=2", Some(&owner)).await;
    assert_eq!(first.times(), [10, 20]);
    let next = first.next.expect("a full page names the next");
    assert_eq!(next, "since=20&skip=1");
    let second = get(&organization, &audit, &format!("/admin/audit?{next}&limit=2"), Some(&owner)).await;
    assert_eq!(second.times(), [20, 30]);
}
