#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use app::{Client, Decision, EngineEvent, Error, Image, MemoryKind, Model, ModelProvider};
use app::{InMemorySecretStore, LocalServices};
use domain::{AgentAsk, AskId, AskKind, ChoreAnswer, SessionCommand, SessionId};
use serde_json::json;

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn script(extra: Vec<EngineEvent>) -> Vec<EngineEvent> {
    let mut events = vec![EngineEvent::SessionStarted(SessionId("s-1".into()))];
    events.extend(extra);
    events.push(EngineEvent::TextDelta("Hello ".into()));
    events.push(EngineEvent::TextDelta("there".into()));
    events.push(EngineEvent::Completed { duration_ms: 1, cost_usd: None });
    events
}

fn services(engine: &FakeEngine) -> LocalServices {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.off.insert(domain::features::MEMORY_SEARCH.into());
    let secrets = Arc::new(InMemorySecretStore::default());
    services.providers = services.providers.with_secrets(secrets);
    services
}

fn build(engine: &FakeEngine, data: &std::path::Path) -> Client {
    Client::builder().data_dir(data).services(services(engine)).build().expect(brand::COMMAND)
}

#[test]
fn a_session_streams_a_run_and_the_next_prompt_resumes_it() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(Vec::new()));
    let client = build(&engine, data.path());
    assert_eq!(client.models().expect("models").iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["scripted"]);

    let session = client.session(data.path(), model());
    assert_eq!(session.send("hi").expect("run").text().expect("text"), "Hello there");
    assert_eq!(session.id(), Some(SessionId("s-1".into())));
    let events: Vec<EngineEvent> = session.send("again").expect("run").collect();
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })));

    let requests = engine.requests();
    assert_eq!(requests[0].session.resume, None);
    assert_eq!(requests[1].session.resume, Some(SessionId("s-1".into())), "the session continues");
    assert!(requests[0].host.memory.is_some(), "memory is on by default");
}

#[test]
fn images_sent_with_a_prompt_reach_the_run() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(Vec::new()));
    let client = build(&engine, data.path());
    let image = Image::from_bytes(b"\x89PNG\r\n\x1a\n pixels").expect("ok").expect("png");
    let session = client.session(data.path(), model());
    session.send_with_images("what is this?", vec![image.clone()]).expect("run").text().expect("text");
    assert_eq!(engine.requests()[0].prompt.images, [image]);
}

#[test]
fn a_question_is_answered_through_the_run() {
    let data = tempfile::tempdir().expect("data");
    let ask = AgentAsk {
        id: AskId("a-1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: false, rule: None },
    };
    let engine = FakeEngine::new(vec![model()], script(vec![EngineEvent::Asked(ask)]))
        .with_delay(std::time::Duration::from_millis(40));
    let client = build(&engine, data.path());
    let mut run = client.session(data.path(), model()).send("list files").expect("run");
    while let Some(event) = run.next_event() {
        if let EngineEvent::Asked(ask) = event {
            run.answer(ask.id, Decision::Allow).expect("answer");
        }
    }
    assert!(run.is_done());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while engine.answers().is_empty() && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(engine.answers(), [(AskId("a-1".into()), Decision::Allow)]);
}

#[test]
fn memory_can_be_written_listed_and_searched() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let client = build(&engine, data.path());
    let memory = client.memory(data.path());
    assert!(memory.list().expect("list").is_empty());
    memory.remember(MemoryKind::Semantic, "The renderer uses wgpu.", &["gpu"]).expect("remember");
    let listed = memory.remember(MemoryKind::Procedural, "Run tests with nextest.", &[]).expect("remember");
    assert_eq!(listed.len(), 2);
    let found = memory.search("which renderer").expect("search");
    assert_eq!(found.first().map(|(m, _)| m.text.as_str()), Some("The renderer uses wgpu."));
    let left = memory.forget(listed[0].id).expect("forget");
    assert_eq!(left.len(), 1);
    let forgotten = memory.forgotten().expect("forgotten");
    assert_eq!(forgotten.iter().map(|f| f.memory.id).collect::<Vec<_>>(), vec![listed[0].id]);
    assert_eq!(memory.restore(listed[0].id).expect("restore").len(), 2);
    assert!(memory.forgotten().expect("forgotten").is_empty());
}

#[test]
fn memory_is_optimized_by_a_run() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
    let client = build(&engine, data.path());
    let memory = client.memory(data.path());
    memory.remember(MemoryKind::Semantic, "The renderer uses wgpu.", &[]).expect("remember");
    let mut run = memory.optimize(model()).expect("optimize");
    while run.next_event().is_some() {}
    let request = engine.requests().pop().expect("a run");
    assert!(request.prompt.text.contains("[0] semantic, from the user: The renderer uses wgpu."));
    assert!(matches!(memory.undo(), Err(Error::Feature(failed)) if failed.to_string().contains("nothing to undo")));
}

#[test]
fn providers_are_listed_and_a_bad_key_is_refused() {
    let data = tempfile::tempdir().expect("data");
    agent_engine::providers::install_testing();
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let client = build(&engine, data.path());
    assert!(client.providers().expect("providers").iter().all(|p| p.account.is_none()));
    assert!(client.connect_provider(ModelProvider::new("keyed"), "  ").is_err());
    let listed = client.connect_provider(ModelProvider::new("keyed"), "sk-key-0123456789abcd").expect("connect");
    assert!(listed.iter().any(|p| p.provider == ModelProvider::new("keyed") && p.account.is_some()));
}

#[test]
fn settings_are_kept_between_builds() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let client = build(&engine, data.path());
    assert_eq!(client.settings().expect("settings"), json!({}));
    client.save_settings(json!({"model": "scripted"})).expect("save");
    assert_eq!(client.settings().expect("settings"), json!({"model": "scripted"}), "kept by the time the save returns");
    let again = build(&engine, data.path());
    assert_eq!(again.settings().expect("settings"), json!({"model": "scripted"}));
}

#[test]
fn a_failed_settings_save_is_reported_to_the_caller_that_saved() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let client = build(&engine, data.path());
    client.save_settings(json!({"model": "scripted"})).expect("save");

    let reader = client.clone();
    let reading = std::thread::spawn(move || (0..20).map(|_| reader.settings()).collect::<Vec<_>>());
    let refused = client.save_settings(json!({ "notes": "x".repeat(300 * 1024) }));
    let read = reading.join().expect("join");

    assert!(
        matches!(&refused, Err(Error::Backend(message)) if message.contains("Could not save the settings")),
        "{refused:?}"
    );
    assert!(read.iter().all(|settings| matches!(settings, Ok(kept) if *kept == json!({"model": "scripted"}))));
}

#[test]
fn a_session_s_effort_and_mode_reach_the_run_and_a_control_cancels_it_from_elsewhere() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(Vec::new())).with_delay(std::time::Duration::from_millis(100));
    let client = build(&engine, data.path());
    let mut session = client.session(data.path(), model()).memory(false);
    session.set_effort(app::Effort::Max);
    session.set_permission(app::PermissionMode::Plan);
    let run = session.send("think hard").expect("run");
    let control = run.control();
    std::thread::spawn(move || control.cancel().expect("cancel"));
    let events: Vec<EngineEvent> = run.collect();
    assert_eq!(events.last(), Some(&EngineEvent::Cancelled));
    let request = engine.requests().remove(0);
    assert_eq!((request.session.effort, request.policy.permission), (app::Effort::Max, app::PermissionMode::Plan));
    assert!(request.host.memory.is_none(), "memory was turned off");
}

#[test]
fn a_program_manages_the_owner_s_mcp_servers() {
    use app::{McpOrigin, McpState};
    let data = tempfile::tempdir().expect("data");
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir_all(repo.path().join(brand::DIR)).expect("dir");
    std::fs::write(
        repo.path().join(concat!(brand::dir!(), "/mcp.json")),
        r#"{"mcpServers": {"jira": {"command": "jira-mcp"}}}"#,
    )
    .expect("config");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let mut services = services(&engine);
    services.homes = std::sync::Arc::new(app::OwnerHome::at(home.path()));
    let client = Client::builder().data_dir(data.path()).services(services).build().expect(brand::COMMAND);
    let states = |servers: Vec<app::McpServerInfo>| -> Vec<(String, McpOrigin, McpState)> {
        servers.into_iter().map(|s| (s.name, s.origin, s.state)).collect()
    };

    assert_eq!(
        states(client.mcp_servers(repo.path()).expect("list")),
        [("jira".into(), McpOrigin::Project, McpState::Untrusted)],
        "shown as skipped until the folder is trusted"
    );
    assert!(client.approve_mcp_server(repo.path(), "ghost", true).is_err(), "only a server that is there");
    assert_eq!(client.set_trust(repo.path(), true).expect("trust").trust, app::Trust::Trusted);
    assert_eq!(
        states(client.mcp_servers(repo.path()).expect("list")),
        [("jira".into(), McpOrigin::Project, McpState::Pending)]
    );
    let approved = client.approve_mcp_server(repo.path(), "jira", true).expect("approve");
    assert_eq!(states(approved)[0].2, McpState::Ready);
    let added = client
        .add_mcp_server(
            repo.path(),
            "atlassian",
            json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"}),
            false,
        )
        .expect("add");
    assert_eq!(client.mcp_server(repo.path(), "atlassian").expect("get").info.origin, McpOrigin::User);
    assert!(client.mcp_listing(repo.path()).expect("listing").unreadable.is_empty());
    assert_eq!(states(added)[1], ("atlassian".into(), McpOrigin::User, McpState::Ready));
    assert!(home.path().join(concat!(brand::dir!(), "/mcp.json")).is_file());
    assert!(client.add_mcp_server(repo.path(), "plugin_x", json!({"command": "x"}), false).is_err());
    assert_eq!(client.remove_mcp_server(repo.path(), "atlassian").expect("remove").len(), 1);
}

#[test]
fn a_side_question_is_answered_from_the_conversation_without_continuing_it() {
    let data = tempfile::tempdir().expect("data");
    let answer = EngineEvent::Chore(ChoreAnswer::Side("It edits main.rs.".into()));
    let engine = FakeEngine::new(vec![model()], script(vec![answer]));
    let client = build(&engine, data.path());
    let session = client.session(data.path(), model());
    assert!(matches!(session.side("too early?"), Err(app::Error::NothingToAsk)));

    session.send("fix the bug").expect("run").text().expect("text");
    assert_eq!(session.side("which file?").expect("run").side_answer().expect("answer"), "It edits main.rs.");

    let side = engine.requests().remove(1);
    assert_eq!(side.session.command, Some(SessionCommand::Side));
    assert_eq!(side.prompt.text, "which file?");
    assert_eq!(side.session.resume, Some(SessionId("s-1".into())), "asked about this conversation");
    assert!(side.host.memory.is_none(), "a side question recalls no memory");
}

#[test]
fn side_calls_name_suggest_recap_and_warn_about_a_conversation() {
    let data = tempfile::tempdir().expect("data");
    let answer = EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into()));
    let engine = FakeEngine::new(vec![model()], script(vec![answer]));
    let client = build(&engine, data.path());
    let session = client.session(data.path(), model());
    assert!(matches!(session.recap(), Err(app::Error::NothingToAsk)));
    let title = session.title("fix the login page").expect("a title needs no conversation");
    assert_eq!(title.chore_answer().expect("answer"), Some(ChoreAnswer::Suggestion("run the tests".into())));

    session.send("fix the bug").expect("run").text().expect("text");
    for run in [session.suggest(), session.recap(), session.heads_up()] {
        run.expect("run").chore_answer().expect("answer");
    }
    let requests = engine.requests();
    let asked: Vec<_> = requests.iter().map(|r| (r.session.command, r.session.resume.is_some())).collect();
    assert_eq!(
        asked,
        [
            (Some(SessionCommand::Title), false),
            (None, false),
            (Some(SessionCommand::Suggest), true),
            (Some(SessionCommand::Recap), true),
            (Some(SessionCommand::HeadsUp), true),
        ]
    );
    assert_eq!(requests[0].prompt.text, "fix the login page");
}

#[test]
fn a_session_s_model_roles_reach_every_run() {
    use domain::model_roles::{ModelRole, ModelRoles};
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(vec![EngineEvent::TextDelta("ok".into())]));
    let client = build(&engine, data.path());
    let mut session = client.session(data.path(), model()).memory(false);
    let mut roles = ModelRoles::default();
    roles.set(ModelRole::Explore, Some("grok-subscription:grok-4.6-fast".into()));
    session.set_roles(roles.clone());
    let _: Vec<EngineEvent> = session.send("look around").expect("run").collect();
    assert_eq!(engine.requests().remove(0).session.roles, roles);
}
