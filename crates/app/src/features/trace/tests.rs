use agent_engine::sessions::{Entry, Sessions};
use domain::trace::insights::DAY_MS;
use domain::trace::runs::SpanKind;
use domain::{EngineEvent, ToolCall};

use super::*;

const NOW: u64 = 400 * DAY_MS;

fn record(store: &FileSessions, dir: &Path, id: &str, at_ms: u64) -> SessionId {
    let id = SessionId(id.into());
    let request = dir.to_path_buf();
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
    let current = record(&store, dir.path(), "s1", NOW - DAY_MS);
    let recent = record(&store, dir.path(), "s2", NOW - 2 * DAY_MS);
    let stale = record(&store, dir.path(), "s3", NOW - 40 * DAY_MS);
    let others = [recent, stale, SessionId("missing".into()), current.clone()];

    let report = reported(&store, dir.path(), &current, &others, NOW).expect("report");

    let ids: Vec<&str> = report.sessions.iter().map(|session| session.id.as_str()).collect();
    assert_eq!(ids, ["s1", "s2"]);
    let run = &report.sessions[0].runs[0];
    assert_eq!(run.prompt, "read it in s1");
    let tool = run.spans.iter().find(|span| span.kind == SpanKind::Tool).expect("tool");
    assert_eq!((tool.start_ms, tool.end_ms, tool.output.as_str()), (1, 3, "alpha"));
    assert_eq!((report.current.as_str(), report.insights.totals.runs), ("s1", 2));
}

#[test]
fn a_live_update_carries_only_the_session_being_followed() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let id = record(&store, dir.path(), "s1", NOW);
    let update = followed(&store, dir.path(), &id, NOW).expect("update");
    assert_eq!(update["id"], "s1");
    assert_eq!(update["insights"]["totals"]["runs"], 1, "the session's own overview comes with it");
    assert_eq!(update["runs"][0]["prompt"], "read it in s1");
    assert_eq!(update["runs"][0]["status"], "running");
}

#[test]
fn another_folder_finds_nothing_to_trace() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let id = record(&store, dir.path(), "s1", NOW);
    assert!(reported(&store, &dir.path().join("elsewhere"), &id, &[], NOW).is_err());
}

#[test]
fn the_report_of_every_session_reads_each_from_its_own_folder_newest_first() {
    let root = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(root.path().join("sessions"));
    let (app, lib) = (root.path().join("app"), root.path().join("lib"));
    let older = record(&store, &app, "a1", NOW - 3 * DAY_MS);
    let newer = record(&store, &lib, "l1", NOW - DAY_MS);
    let stale = record(&store, &app, "a0", NOW - 40 * DAY_MS);
    let listed = [(older, app.clone()), (newer, lib.clone()), (stale, app), (SessionId("gone".into()), lib)];

    let report = everything(&store, &listed, NOW);

    let ids: Vec<&str> = report.sessions.iter().map(|session| session.id.as_str()).collect();
    assert_eq!(ids, ["l1", "a1"]);
    assert_eq!(report.current, "l1");
    assert_eq!(report.insights.totals.runs, 2);
}

#[test]
fn with_no_session_at_all_the_report_is_empty() {
    let root = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(root.path().join("sessions"));
    let report = everything(&store, &[], NOW);
    assert!(report.sessions.is_empty());
    assert_eq!(report.current, "");
}

#[test]
fn a_followed_session_keeps_every_text_whole_while_the_report_stays_light() {
    let root = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(root.path().join("sessions"));
    let id = SessionId("long".into());
    let request = root.path().to_path_buf();
    let long = "x".repeat(30_000);
    for entry in [
        Entry::Prompted { at_ms: NOW, prompt: long.clone() },
        Entry::Event { at_ms: NOW + 1, event: EngineEvent::ToolStarted(ToolCall::new("r", "Read", "a.txt")) },
        Entry::Event {
            at_ms: NOW + 2,
            event: EngineEvent::ToolResult { id: "r".into(), output: long.clone(), failed: false },
        },
    ] {
        store.record(&request, &id, &entry).expect("record");
    }

    let followed = traced(&store, root.path(), &id).expect("runs");
    let tool = followed[0].spans.iter().find(|span| span.kind == SpanKind::Tool).expect("tool");
    assert_eq!(tool.output.len(), 30_000);

    let report = everything(&store, &[(id, root.path().to_path_buf())], NOW);
    let tool = report.sessions[0].runs[0].spans.iter().find(|span| span.kind == SpanKind::Tool).expect("tool");
    assert!(tool.output.len() < 30_000);
}
