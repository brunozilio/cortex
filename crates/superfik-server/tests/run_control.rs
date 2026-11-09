//! A run answers only to whoever started it: another member who learns its
//! id may neither steer it nor follow it, and every member's command, even
//! those the server answers itself, meets their role first.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::policy::Policy;
use superfik_core::{
    AgentAsk, AskId, AskKind, Decision, Effort, EngineEvent, EngineKind, Member, ModelOption, PermissionMode,
    Principal, PromptRequest, RunId, WorkspaceId,
};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Organization, Server};

const NOW: u64 = 1_800_000_000;

struct Fixture {
    organization: Arc<Organization>,
    hosted: Hosted,
    engine: FakeEngine,
    server: Server,
}

fn ask() -> AgentAsk {
    AgentAsk {
        id: AskId("r1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: false },
    }
}

fn model() -> ModelOption {
    ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5")
}

fn fixture() -> Fixture {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let org = organization.directory.org();
    let script = vec![
        EngineEvent::Asked(ask()),
        EngineEvent::TextDelta("Hello".into()),
        EngineEvent::Completed { duration_ms: 10, cost_usd: None },
    ];
    let engine = FakeEngine::new(EngineKind::Native, vec![model()], script).with_delay(Duration::from_millis(150));
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine.clone())));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.plugins = superfik_backend::Plugins::new(
        Arc::new(superfik_backend::InMemoryPluginStore::new(tempfile::tempdir().expect("packages").keep())),
        Arc::new(superfik_backend::InMemorySecretStore::default()),
        Arc::new(superfik_backend::McpOAuth::default()),
        Arc::new(superfik_backend::SystemTokenCommand),
    );
    let roles = superfik_server::DirectoryRoles(Arc::clone(&organization));
    services.authorizer = Arc::new(superfik_backend::RoleAuthorizer::new(org, roles));
    let hosted = Hosted::in_memory(Arc::clone(&organization));
    let server = Server::start_for(services, Some(hosted.clone())).expect("server");
    Fixture { organization, hosted, engine, server }
}

impl Fixture {
    fn join(&self, name: &str, role: OrgRole) -> (Member, Connection) {
        let member = admit(&self.organization, name, role).0;
        (member, self.server.connect(Principal::Member(member), Arc::new(|| {})))
    }

    fn request(&self, member: Member) -> PromptRequest {
        PromptRequest {
            workspace: WorkspaceId::new(),
            cwd: self.hosted.checkouts.home(member.user).join("rocket"),
            prompt: "Ship it".into(),
            model: model(),
            effort: Effort::High,
            output_style: None,
            safety_check: true,
            repositories: vec![],
            skills: vec![],
            disabled_skills: vec![],
            references: vec![],
            notes: vec![],
            resume: None,
            command: None,
            handoff: None,
            memory: None,
            permission: PermissionMode::Ask,
            connections: vec![],
            plugins: None,
            browser: None,
            credentials: Default::default(),
        }
    }
}

fn send(connection: &Connection, command: ClientCommand) {
    connection.send(Envelope::local(command)).expect("send");
}

fn until(connection: &Connection, done: impl Fn(&ServerEvent) -> bool) -> Vec<ServerEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    loop {
        if let Some(event) = connection.try_recv() {
            let finished = done(&event);
            events.push(event);
            if finished {
                return events;
            }
        }
        assert!(Instant::now() < deadline, "timed out; got {events:?}");
        std::thread::yield_now();
    }
}

fn refusals(connection: &Connection, count: usize) -> Vec<String> {
    let mut messages = Vec::new();
    while messages.len() < count {
        match until(connection, |event| matches!(event, ServerEvent::Error { .. })).pop() {
            Some(ServerEvent::Error { message }) => messages.push(message),
            other => panic!("not refused: {other:?}"),
        }
    }
    messages
}

#[test]
fn another_member_can_neither_steer_nor_follow_a_run() {
    let f = fixture();
    let (olga, olga_app) = f.join("Olga", OrgRole::Member);
    let (max, max_app) = f.join("Max", OrgRole::Member);
    let run = RunId::new();
    send(&olga_app, ClientCommand::SubmitPrompt { run, request: f.request(olga) });
    until(&olga_app, |event| matches!(event, ServerEvent::Run { event: EngineEvent::Asked(_), .. }));

    send(&max_app, ClientCommand::CancelRun { run });
    send(&max_app, ClientCommand::AnswerAsk { run, ask: ask().id, decision: Decision::Allow });
    send(&max_app, ClientCommand::SetPermissionMode { run, mode: PermissionMode::Bypass });
    send(&max_app, ClientCommand::SubmitPrompt { run, request: f.request(max) });
    let refused = refusals(&max_app, 4);
    assert!(refused.iter().all(|message| message.contains("may not")), "{refused:?}");

    send(&olga_app, ClientCommand::AnswerAsk { run, ask: ask().id, decision: Decision::Deny });
    let rest = until(&olga_app, ServerEvent::is_run_terminal);
    assert_eq!(
        rest.last(),
        Some(&ServerEvent::Run { run, event: EngineEvent::Completed { duration_ms: 10, cost_usd: None } })
    );
    assert_eq!(f.engine.answers(), [(ask().id, Decision::Deny)], "only the owner's answer reached the run");
    std::thread::sleep(Duration::from_millis(50));
    let heard: Vec<ServerEvent> = std::iter::from_fn(|| max_app.try_recv()).collect();
    assert!(!heard.iter().any(|event| matches!(event, ServerEvent::Run { .. })), "{heard:?}");
}

#[test]
fn the_policy_caps_a_runs_mode_once_it_runs() {
    let f = fixture();
    let (olga, olga_app) = f.join("Olga", OrgRole::Member);
    f.hosted
        .governance
        .set_policy(&Policy { max_mode: Some(PermissionMode::Ask), ..Policy::default() })
        .expect("policy");
    let run = RunId::new();
    send(&olga_app, ClientCommand::SubmitPrompt { run, request: f.request(olga) });
    until(&olga_app, |event| matches!(event, ServerEvent::Run { event: EngineEvent::Asked(_), .. }));
    send(&olga_app, ClientCommand::SetPermissionMode { run, mode: PermissionMode::Bypass });
    let refused = refusals(&olga_app, 1);
    assert!(refused[0].contains("policy"), "{refused:?}");
}

fn answered_by_the_server(home: PathBuf) -> Vec<ClientCommand> {
    vec![
        ClientCommand::Viewing { session: None },
        ClientCommand::Upload { from: home.join("sky.png"), name: "sky.png".into(), data: "c2t5".into() },
        ClientCommand::LocateRepository { slug: "acme/rocket".into(), known: vec![], clone_root: "/x".into() },
        ClientCommand::SignInToGitHub,
        ClientCommand::SignOutOfGitHub,
    ]
}

#[test]
fn guests_and_removed_members_are_refused_what_the_server_answers_itself() {
    let f = fixture();
    let (gil, gil_app) = f.join("Gil", OrgRole::Guest);
    let (rita, rita_app) = f.join("Rita", OrgRole::Member);
    f.organization.directory.remove(superfik_identity::Actor::Console, rita.user).expect("removed");
    for (member, app) in [(gil, &gil_app), (rita, &rita_app)] {
        let commands = answered_by_the_server(f.hosted.checkouts.home(member.user));
        let count = commands.len();
        for command in commands {
            send(app, command);
        }
        let refused = refusals(app, count);
        assert!(refused.iter().all(|message| message.contains("may not")), "{refused:?}");
    }
}
