use agent_engine::sessions::{Entry, Sessions};
use domain::trace::insights::DAY_MS;
use domain::trace::runs::SpanKind;
use domain::{EngineEvent, OrgId, ToolCall, UserId};

use super::*;

const NOW: u64 = 400 * DAY_MS;

fn record(store: &FileSessions, dir: &Path, owner: &Principal, id: &str, at_ms: u64) -> SessionId {
    let id = SessionId(id.into());
    let mut request = PromptRequest::plain(dir.into(), "", ModelOption::new("", ""));
    request.credentials.owner = Some(owner.clone());
    for entry in [
        Entry::Prompted { at_ms, prompt: format!("read it in {}", id.0) },
        Entry::Event { at_ms: at_ms + 1, event: EngineEvent::ToolStarted(ToolCall::new("r", "Read", "a.txt")) },
        Entry::Event {
            at_ms: at_ms + 3,
            event: EngineEvent::ToolResult { id: "r".into(), output: "alpha".into(), failed: false },
        },
    ] {
        store.record(&request, &id, &entry).expect("record");
    }
    id
}

#[test]
fn the_report_leads_with_the_session_and_adds_the_projects_recent_ones() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let owner = Principal::LocalOwner;
    let current = record(&store, dir.path(), &owner, "s1", NOW - DAY_MS);
    let recent = record(&store, dir.path(), &owner, "s2", NOW - 2 * DAY_MS);
    let stale = record(&store, dir.path(), &owner, "s3", NOW - 40 * DAY_MS);
    let others = [recent, stale, SessionId("missing".into()), current.clone()];

    let report = reported(&store, &owner, dir.path(), &current, &others, NOW).expect("report");

    let ids: Vec<&str> = report.sessions.iter().map(|session| session.id.as_str()).collect();
    assert_eq!(ids, ["s1", "s2"]);
    let run = &report.sessions[0].runs[0];
    assert_eq!(run.prompt, "read it in s1");
    let tool = run.spans.iter().find(|span| span.kind == SpanKind::Tool).expect("tool");
    assert_eq!((tool.start_ms, tool.end_ms, tool.output.as_str()), (1, 3, "alpha"));
    assert_eq!((report.current.as_str(), report.insights.totals.runs), ("s1", 2));
}

#[test]
fn another_principal_or_folder_finds_nothing_to_trace() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let id = record(&store, dir.path(), &Principal::LocalOwner, "s1", NOW);
    let stranger = Principal::member(UserId::new(), OrgId::new());
    assert!(reported(&store, &stranger, dir.path(), &id, &[], NOW).is_err());
    assert!(reported(&store, &Principal::LocalOwner, &dir.path().join("elsewhere"), &id, &[], NOW).is_err());
}
