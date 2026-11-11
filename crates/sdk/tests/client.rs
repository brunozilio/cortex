//! The SDK end to end on a real `LocalBackend` with a scripted engine: no
//! network, no secrets on disk, no model download.

use std::sync::Arc;

use serde_json::json;
use backend::{InMemorySecretStore, InMemoryStore, LocalServices, Memories};
use domain::{AgentAsk, AskId, AskKind, ChoreAnswer, SessionCommand, SessionId};
use agent_engine::fake::FakeEngine;
use sdk::{
    Agent, Command, Decision, EngineEvent, Error, Image, McpConnection, McpServer, MemoryKind, Model, ModelProvider,
    Skill, Superfik,
};

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
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let secrets = Arc::new(InMemorySecretStore::default());
    services.providers = services.providers.with_secrets(secrets);
    services
}

fn build(engine: &FakeEngine, data: &std::path::Path) -> Superfik {
    Superfik::builder().data_dir(data).services(services(engine)).build().expect("superfik")
}

#[test]
fn a_session_streams_a_run_and_the_next_prompt_resumes_it() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(Vec::new()));
    let superfik = build(&engine, data.path());
    assert_eq!(superfik.models().expect("models").iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["scripted"]);

    let session = superfik.session(data.path(), model());
    assert_eq!(session.send("hi").expect("run").text().expect("text"), "Hello there");
    assert_eq!(session.id(), Some(SessionId("s-1".into())));
    let events: Vec<EngineEvent> = session.send("again").expect("run").collect();
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })));

    let requests = engine.requests();
    assert_eq!(requests[0].resume, None);
    assert_eq!(requests[1].resume, Some(SessionId("s-1".into())), "the session continues");
    assert!(requests[0].memory.is_some(), "memory is on by default");
}

#[test]
fn images_sent_with_a_prompt_reach_the_run() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(Vec::new()));
    let superfik = build(&engine, data.path());
    let image = Image::from_bytes(b"\x89PNG\r\n\x1a\n pixels").expect("ok").expect("png");
    let session = superfik.session(data.path(), model());
    session.send_with_images("what is this?", vec![image.clone()]).expect("run").text().expect("text");
    assert_eq!(engine.requests()[0].images, [image]);
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
    let superfik = build(&engine, data.path());
    let mut run = superfik.session(data.path(), model()).send("list files").expect("run");
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
fn agents_skills_commands_and_mcp_servers_reach_every_run() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(Vec::new()));
    let superfik = Superfik::builder()
        .data_dir(data.path())
        .name("app")
        .agent(Agent::new("reviewer", "Reviews diffs.").instructions("Review."))
        .skill(Skill::new("release", "Cuts a release.", "Tag it."))
        .command(Command::new("audit", "Audits a path.", "Audit $ARGUMENTS"))
        .mcp(McpServer::new("math").tool("add", "Adds.", json!({"type": "object"}), |_| Ok(json!(3))))
        .connect(McpConnection::http("docs", "https://docs.test/mcp").header("Authorization", "Bearer t"))
        .services(services(&engine))
        .build()
        .expect("superfik");
    let skills: Vec<String> = superfik.skills(data.path()).expect("skills").into_iter().map(|s| s.name).collect();
    assert!(skills.contains(&"release".to_owned()) && skills.contains(&"audit".to_owned()), "{skills:?}");
    let agents = superfik.agents(data.path()).expect("agents");
    assert!(agents.iter().any(|agent| agent.name == "reviewer"));

    superfik.session(data.path(), model()).send("/audit src").expect("run").text().expect("text");
    let request = engine.requests().remove(0);
    let brief = request.extensions.expect("brief");
    assert_eq!(brief.dirs, [data.path().join("sdk").join("app")]);
    assert!(brief.dirs[0].join("commands/audit.md").is_file());
    assert_eq!(request.credentials.mcp.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["math"]);
    let config = brief.mcp_config.expect("the HTTP server joins the run's MCP config");
    assert!(!config.exists(), "the run's private MCP config is removed when it ends");
}

#[test]
fn memory_can_be_written_listed_and_searched() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let superfik = build(&engine, data.path());
    let memory = superfik.memory(data.path());
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
    let superfik = build(&engine, data.path());
    let memory = superfik.memory(data.path());
    memory.remember(MemoryKind::Semantic, "The renderer uses wgpu.", &[]).expect("remember");
    let mut run = memory.optimize(model()).expect("optimize");
    while run.next_event().is_some() {}
    let request = engine.requests().pop().expect("a run");
    assert!(request.prompt.contains("[0] semantic: The renderer uses wgpu."));
    assert!(matches!(memory.undo(), Err(Error::Backend(message)) if message.contains("nothing to undo")));
}

#[test]
fn providers_are_listed_and_a_bad_key_is_refused() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let superfik = build(&engine, data.path());
    assert!(superfik.providers().expect("providers").iter().all(|p| p.account.is_none()));
    assert!(superfik.connect_provider(ModelProvider::Anthropic, "  ").is_err());
    let listed = superfik.connect_provider(ModelProvider::Anthropic, "sk-ant-0123456789abcd").expect("connect");
    assert!(listed.iter().any(|p| p.provider == ModelProvider::Anthropic && p.account.is_some()));
}

#[test]
fn settings_are_kept_between_builds() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let superfik = build(&engine, data.path());
    assert_eq!(superfik.settings().expect("settings"), json!({}));
    superfik.save_settings(json!({"model": "scripted"})).expect("save");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while superfik.settings().expect("settings") != json!({"model": "scripted"}) {
        assert!(std::time::Instant::now() < deadline, "the settings were not saved");
    }
    let again = build(&engine, data.path());
    assert_eq!(again.settings().expect("settings"), json!({"model": "scripted"}));
}

#[test]
fn a_session_s_effort_and_mode_reach_the_run_and_a_control_cancels_it_from_elsewhere() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], script(Vec::new())).with_delay(std::time::Duration::from_millis(100));
    let superfik = build(&engine, data.path());
    let mut session = superfik.session(data.path(), model()).memory(false);
    session.set_effort(sdk::Effort::Max);
    session.set_permission(sdk::PermissionMode::Plan);
    let run = session.send("think hard").expect("run");
    let control = run.control();
    std::thread::spawn(move || control.cancel().expect("cancel"));
    let events: Vec<EngineEvent> = run.collect();
    assert_eq!(events.last(), Some(&EngineEvent::Cancelled));
    let request = engine.requests().remove(0);
    assert_eq!((request.effort, request.permission), (sdk::Effort::Max, sdk::PermissionMode::Plan));
    assert!(request.memory.is_none(), "memory was turned off");
}

#[test]
fn a_program_manages_the_owner_s_mcp_servers() {
    use sdk::{McpOrigin, McpState};
    let data = tempfile::tempdir().expect("data");
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir_all(repo.path().join(".superfik")).expect("dir");
    std::fs::write(repo.path().join(".superfik/mcp.json"), r#"{"mcpServers": {"jira": {"command": "jira-mcp"}}}"#)
        .expect("config");
    let engine = FakeEngine::new(vec![model()], Vec::new());
    let mut services = services(&engine);
    services.home = Some(home.path().to_path_buf());
    let superfik = Superfik::builder().data_dir(data.path()).services(services).build().expect("superfik");
    let states = |servers: Vec<sdk::McpServerInfo>| -> Vec<(String, McpOrigin, McpState)> {
        servers.into_iter().map(|s| (s.name, s.origin, s.state)).collect()
    };

    assert_eq!(
        states(superfik.mcp_servers(repo.path()).expect("list")),
        [("jira".into(), McpOrigin::Project, McpState::Pending)]
    );
    let approved = superfik.approve_mcp_server(repo.path(), "jira", true).expect("approve");
    assert_eq!(states(approved)[0].2, McpState::Ready);
    let added = superfik
        .add_mcp_server(repo.path(), "atlassian", json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"}))
        .expect("add");
    assert_eq!(states(added)[1], ("atlassian".into(), McpOrigin::User, McpState::Ready));
    assert!(home.path().join(".superfik/mcp.json").is_file());
    assert!(superfik.add_mcp_server(repo.path(), "plugin_x", json!({"command": "x"})).is_err());
    assert_eq!(superfik.remove_mcp_server(repo.path(), "atlassian").expect("remove").len(), 1);
}

#[test]
fn a_side_question_is_answered_from_the_conversation_without_continuing_it() {
    let data = tempfile::tempdir().expect("data");
    let answer = EngineEvent::Chore(ChoreAnswer::Side("It edits main.rs.".into()));
    let engine = FakeEngine::new(vec![model()], script(vec![answer]));
    let superfik = build(&engine, data.path());
    let session = superfik.session(data.path(), model());
    assert!(matches!(session.side("too early?"), Err(sdk::Error::NothingToAsk)));

    session.send("fix the bug").expect("run").text().expect("text");
    assert_eq!(session.side("which file?").expect("run").side_answer().expect("answer"), "It edits main.rs.");

    let side = engine.requests().remove(1);
    assert_eq!(side.command, Some(SessionCommand::Side));
    assert_eq!(side.prompt, "which file?");
    assert_eq!(side.resume, Some(SessionId("s-1".into())), "asked about this conversation");
    assert!(side.memory.is_none(), "a side question recalls no memory");
}

#[test]
fn side_calls_name_suggest_recap_and_warn_about_a_conversation() {
    let data = tempfile::tempdir().expect("data");
    let answer = EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into()));
    let engine = FakeEngine::new(vec![model()], script(vec![answer]));
    let superfik = build(&engine, data.path());
    let session = superfik.session(data.path(), model());
    assert!(matches!(session.recap(), Err(sdk::Error::NothingToAsk)));
    let title = session.title("fix the login page").expect("a title needs no conversation");
    assert_eq!(title.chore_answer().expect("answer"), Some(ChoreAnswer::Suggestion("run the tests".into())));

    session.send("fix the bug").expect("run").text().expect("text");
    for run in [session.suggest(), session.recap(), session.heads_up()] {
        run.expect("run").chore_answer().expect("answer");
    }
    let requests = engine.requests();
    let asked: Vec<_> = requests.iter().map(|r| (r.command, r.resume.is_some())).collect();
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
    assert_eq!(requests[0].prompt, "fix the login page");
}
