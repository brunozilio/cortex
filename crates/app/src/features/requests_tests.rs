use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::features::channels::{ChannelsReply, ChannelsRequest};
use crate::api::features::memory::{MemoryOp, MemoryReply, MemoryRequest};
use crate::api::features::sessions::{SessionsReply, SessionsRequest};
use crate::api::features::trace::{TraceReply, TraceRequest};
use crate::api::{FeatureError, FeatureName, FeatureReply, FeatureRequest};
use agent_engine::sessions::{Entry, Sessions};
use domain::SessionId;
use serde_json::json;

use crate::features::context::testing;
use crate::services::LocalServices;

fn typed(services: &LocalServices, client: u64, request: FeatureRequest) -> Result<FeatureReply, FeatureError> {
    testing::with(client, services, |context| super::handle(context, request))
}

fn prompted(services: &LocalServices, cwd: &Path, id: &str) -> SessionId {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
    let id = SessionId(id.into());
    services.sessions.record(cwd, &id, &Entry::Prompted { at_ms: now, prompt: "hi".into() }).expect("record");
    services.catalog.update(&mut |catalog| catalog.started(&id, cwd, now));
    id
}

#[test]
fn sessions_are_listed_read_and_tidied() {
    let services = testing::services(None);
    let id = SessionId("s1".into());
    services.catalog.update(&mut |catalog| catalog.started(&id, Path::new("/repo"), 1));
    let sessions = |request: SessionsRequest| match typed(&services, 1, FeatureRequest::Sessions(request)) {
        Ok(FeatureReply::Sessions(reply)) => reply,
        other => panic!("{other:?}"),
    };
    let (session, cwd) = (id.clone(), std::path::PathBuf::from("/repo"));

    assert_eq!(sessions(SessionsRequest::Length { session: id.clone(), cwd: cwd.clone() }), SessionsReply::Length(0));
    assert_eq!(sessions(SessionsRequest::History { session: id.clone(), cwd }), SessionsReply::History(Vec::new()));
    assert_eq!(
        sessions(SessionsRequest::Rename { session: id.clone(), title: "Greeting".into() }),
        SessionsReply::Changed
    );
    sessions(SessionsRequest::Seen { session: id });
    let SessionsReply::Listed(listed) = sessions(SessionsRequest::List) else { panic!("listed") };
    assert_eq!((listed[0].title.as_deref(), listed[0].unread), (Some("Greeting"), false));
    sessions(SessionsRequest::Delete { session });
    assert_eq!(sessions(SessionsRequest::List), SessionsReply::Listed(Vec::new()));
}

#[test]
fn memory_answers_each_operation_with_its_own_reply_and_an_empty_query_finds_nothing() {
    let data = tempfile::tempdir().expect("data");
    let project = tempfile::tempdir().expect("project");
    let services = testing::services(Some(data.path()));
    let path = project.path().to_path_buf();
    let memory = |op: MemoryOp| {
        let request = FeatureRequest::Memory(MemoryRequest { path: path.clone(), op });
        typed(&services, 1, request).map(|reply| match reply {
            FeatureReply::Memory(reply) => reply,
            other => panic!("{other:?}"),
        })
    };
    let fact = domain::MemoryEdit::Remember {
        kind: domain::MemoryKind::Semantic,
        text: "Uses wgpu".into(),
        tags: Vec::new(),
        touches: Vec::new(),
    };

    let edited = memory(MemoryOp::Edit { edits: vec![fact] }).expect("edited");
    assert!(matches!(&edited, MemoryReply::Listing(listing) if listing.memories[0].text == "Uses wgpu"), "{edited:?}");
    assert_eq!(memory(MemoryOp::List).expect("listed"), edited);
    let nothing = memory(MemoryOp::Search { query: String::new() }).expect("searched");
    assert_eq!(nothing, MemoryReply::Found { path: path.clone(), query: String::new(), scores: Vec::new() });
    let failed = memory(MemoryOp::Check).expect_err("not a repository");
    assert!(matches!(failed, FeatureError::Failed { feature: FeatureName::Memory, .. }), "{failed:?}");
    assert!(memory(MemoryOp::Share { id: domain::MemoryId(9) }).is_err(), "no such memory");
    assert!(memory(MemoryOp::KeepSeeds { picked: Vec::new() }).is_err(), "nothing picked");
    assert!(matches!(memory(MemoryOp::Score), Ok(MemoryReply::Score { path: scored, .. }) if scored == path));
    assert_eq!(memory(MemoryOp::Seed), Ok(MemoryReply::Seeds { path: path.clone(), candidates: Vec::new() }));
}

#[test]
fn the_trace_reports_every_session_or_one_with_its_project_or_follows_only_one() {
    let data = tempfile::tempdir().expect("data");
    let project = tempfile::tempdir().expect("project");
    let services = testing::services(Some(data.path()));
    let session = prompted(&services, project.path(), "s1");
    let cwd = project.path().to_path_buf();
    let trace = |request: TraceRequest| match typed(&services, 1, FeatureRequest::Trace(request)) {
        Ok(FeatureReply::Trace(reply)) => Ok(reply),
        Ok(other) => panic!("{other:?}"),
        Err(error) => Err(error),
    };

    let Ok(TraceReply::Report(everything)) = trace(TraceRequest::EverySession) else { panic!("everything") };
    assert_eq!((&everything["project"], &everything["current"]), (&json!(""), &json!("s1")));
    let named = TraceRequest::Session { session: session.clone(), cwd: cwd.clone() };
    let Ok(TraceReply::Report(report)) = trace(named) else { panic!("report") };
    assert_eq!(report["project"], json!(project.path().display().to_string()));
    assert_eq!(report["sessions"][0]["id"], "s1");
    let Ok(TraceReply::Followed(followed)) = trace(TraceRequest::Follow { session, cwd: cwd.clone() }) else {
        panic!("followed")
    };
    assert_eq!((&followed["id"], followed["runs"].as_array().map(Vec::len)), (&json!("s1"), Some(1)));
    assert!(followed["insights"].is_object() && followed.get("project").is_none());

    let unknown = trace(TraceRequest::Follow { session: SessionId("none".into()), cwd });
    assert!(matches!(unknown, Err(FeatureError::Failed { feature: FeatureName::Trace, .. })), "{unknown:?}");
}

#[test]
fn channels_open_none_for_an_empty_list_and_say_which_names_failed() {
    let project = tempfile::tempdir().expect("project");
    let services = testing::services(None);
    let channels = |names: &[&str]| {
        let names = names.iter().map(|name| (*name).to_owned()).collect();
        let request = ChannelsRequest { path: project.path().to_path_buf(), names };
        match typed(&services, 96_101, FeatureRequest::Channels(request)) {
            Ok(FeatureReply::Channels(reply)) => reply,
            other => panic!("{other:?}"),
        }
    };

    assert_eq!(channels(&[]), ChannelsReply::default());
    let missing = channels(&["chat"]);
    assert!(missing.opened.is_empty());
    assert!(missing.failed[0].starts_with("chat:"), "{missing:?}");
    super::forget(96_101);
}

#[test]
fn a_feature_turned_off_says_so_instead_of_answering() {
    let off = testing::services(None).without(domain::features::TRACE).without(domain::features::OUTPUT_SAVINGS);
    let listing = FeatureRequest::Trace(TraceRequest::EverySession);

    let refused = typed(&off, 1, listing.clone()).expect_err("off");
    assert_eq!(refused, FeatureError::FeatureOff { feature: FeatureName::Trace });
    assert_eq!(refused.to_string(), "trace is turned off (/features trace on)");
    let savings = typed(&off, 1, FeatureRequest::OutputSavings).expect_err("off");
    assert_eq!(savings, FeatureError::FeatureOff { feature: FeatureName::OutputSavings });
    assert_eq!(savings.to_string(), "output_savings is turned off (/features experimental-output-savings on)");
    assert!(typed(&testing::services(None), 1, listing).is_ok());
    assert!(typed(&testing::services(None), 1, FeatureRequest::OutputSavings).is_ok());
    let folder_names = testing::services(None).without("sessions").without("output_savings");
    assert!(typed(&folder_names, 1, FeatureRequest::Sessions(SessionsRequest::List)).is_ok());
    assert!(typed(&folder_names, 1, FeatureRequest::OutputSavings).is_ok(), "a folder's name switches nothing");
}
