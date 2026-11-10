//! The SDK end to end on a real `LocalBackend` with a scripted engine: no
//! network, no keychain, no model download.

use std::sync::Arc;

use serde_json::json;
use superfik_backend::{InMemorySecretStore, InMemoryStore, LocalServices, Memories};
use superfik_core::{AgentAsk, AskId, AskKind, EngineKind, SessionId};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_sdk::{
    Agent, Command, Decision, EngineEvent, McpConnection, McpServer, MemoryKind, Model, ModelProvider, Skill, Superfik,
};

fn model() -> Model {
    Model::new(EngineKind::Native, "scripted", "Scripted")
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
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine.clone())));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let secrets = Arc::new(InMemorySecretStore::default());
    services.plugins = services.plugins.with_secrets(secrets.clone());
    services.providers = services.providers.with_secrets(secrets);
    services
}

fn build(engine: &FakeEngine, data: &std::path::Path) -> Superfik {
    Superfik::builder().data_dir(data).services(services(engine)).build().expect("superfik")
}

#[test]
fn a_session_streams_a_run_and_the_next_prompt_resumes_it() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(EngineKind::Native, vec![model()], script(Vec::new()));
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
fn a_question_is_answered_through_the_run() {
    let data = tempfile::tempdir().expect("data");
    let ask = AgentAsk {
        id: AskId("a-1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: false },
    };
    let engine = FakeEngine::new(EngineKind::Native, vec![model()], script(vec![EngineEvent::Asked(ask)]))
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
    let engine = FakeEngine::new(EngineKind::Native, vec![model()], script(Vec::new()));
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
    let brief = request.plugins.expect("brief");
    assert_eq!(brief.local_dirs, [data.path().join("sdk").join("app")]);
    assert!(brief.local_dirs[0].join("commands/audit.md").is_file());
    assert_eq!(request.credentials.mcp.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["math"]);
    let config = brief.mcp_config.expect("the HTTP server joins the run's MCP config");
    assert!(!config.exists(), "the run's private MCP config is removed when it ends");
}

#[test]
fn memory_can_be_written_listed_and_searched() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(EngineKind::Native, vec![model()], Vec::new());
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
}

#[test]
fn providers_are_listed_and_a_bad_key_is_refused() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(EngineKind::Native, vec![model()], Vec::new());
    let superfik = build(&engine, data.path());
    assert!(superfik.providers().expect("providers").iter().all(|p| p.account.is_none()));
    assert!(superfik.connect_provider(ModelProvider::Anthropic, "  ").is_err());
    let listed = superfik.connect_provider(ModelProvider::Anthropic, "sk-ant-0123456789abcd").expect("connect");
    assert!(listed.iter().any(|p| p.provider == ModelProvider::Anthropic && p.account.is_some()));
}
