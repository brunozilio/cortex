//! An organization's server runs everything for its members (ADR 0010),
//! but only inside each member's own checkouts, and every run follows the
//! organization's policy.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::policy::Policy;
use superfik_core::project::ProjectRef;
use superfik_core::{
    Effort, EngineEvent, EngineKind, Member, ModelOption, PermissionMode, Principal, PromptRequest, RunId, UserId,
    WorkspaceId,
};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::checkouts::Checkouts;
use superfik_server::github_login::{DeviceCode, DeviceFlow, Granted};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Server};

const NOW: u64 = 1_800_000_000;

struct FakeCheckouts {
    root: tempfile::TempDir,
    prepared: Mutex<Vec<ProjectRef>>,
    tokens: Mutex<Vec<Option<String>>>,
}

impl Checkouts for FakeCheckouts {
    fn prepare(&self, project: &ProjectRef, member: UserId, token: Option<&str>) -> Result<PathBuf, String> {
        self.prepared.lock().expect("lock").push(project.clone());
        self.tokens.lock().expect("lock").push(token.map(str::to_owned));
        Ok(self.home(member).join("rocket"))
    }

    fn home(&self, member: UserId) -> PathBuf {
        self.root.path().join(member.to_string())
    }
}

struct FakeGitHub;

impl DeviceFlow for FakeGitHub {
    fn begin(&self) -> Result<DeviceCode, String> {
        Ok(DeviceCode {
            device_code: "d".into(),
            user_code: "ABCD-1234".into(),
            url: "https://github.com/login/device".into(),
            interval: Duration::ZERO,
            expires_in: Duration::from_secs(60),
        })
    }
    fn finish(&self, code: &DeviceCode, _now: u64) -> Result<Granted, String> {
        assert_eq!(code.device_code, "d");
        Ok(Granted { token: "ghu_olga".into(), refresh: None, expires_unix: None })
    }
    fn refresh(&self, _: &str, _: u64) -> Result<Granted, String> {
        Err("unused".into())
    }
    fn login(&self, token: &str) -> Result<String, String> {
        (token == "ghu_olga").then(|| "olga-gh".to_owned()).ok_or_else(|| "who?".into())
    }
}

struct Fixture {
    _server: Server,
    olga: Connection,
    home: PathBuf,
    checkouts: Arc<FakeCheckouts>,
    hosted: Hosted,
}

fn fixture() -> Fixture {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let org = organization.directory.org();
    let olga: Member = admit(&organization, "Olga", superfik_core::access::OrgRole::Owner).0;
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let roles = superfik_server::DirectoryRoles(Arc::clone(&organization));
    services.authorizer = Arc::new(superfik_backend::RoleAuthorizer::new(org, roles));
    let checkouts = Arc::new(FakeCheckouts {
        root: tempfile::tempdir().expect("root"),
        prepared: Mutex::default(),
        tokens: Mutex::default(),
    });
    let github = superfik_server::GitHubSignIn::in_memory(Some(Arc::new(FakeGitHub)));
    services.github_tokens = github.tokens.clone();
    let hosted = Hosted { checkouts: checkouts.clone(), github, ..Hosted::in_memory(organization) };
    let server = Server::start_for(services, Some(hosted.clone())).expect("server");
    let home = checkouts.home(olga.user);
    let olga = server.connect(Principal::Member(olga), Arc::new(|| {}));
    Fixture { _server: server, olga, home, checkouts, hosted }
}

fn next(connection: &Connection) -> ServerEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(event) = connection.try_recv() {
            return event;
        }
        assert!(Instant::now() < deadline, "nothing arrived");
        std::thread::yield_now();
    }
}

fn request(cwd: PathBuf) -> PromptRequest {
    PromptRequest {
        workspace: WorkspaceId::new(),
        cwd,
        prompt: "Ship it".into(),
        model: ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
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
        permission: Default::default(),
        connections: vec![],
        plugins: None,
        browser: None,
        credentials: Default::default(),
    }
}

fn failure(event: ServerEvent, run: RunId) -> String {
    let ServerEvent::Run { run: ended, event: EngineEvent::Failed(why) } = event else { panic!("not failed") };
    assert_eq!(ended, run);
    why
}

#[test]
fn a_prompt_in_the_members_own_checkout_reaches_the_workers() {
    let f = fixture();
    let run = RunId::new();
    f.olga
        .send(Envelope::local(ClientCommand::SubmitPrompt { run, request: request(f.home.join("rocket")) }))
        .expect("send");
    let why = failure(next(&f.olga), run);
    assert!(!why.contains("outside"), "{why}");
}

#[test]
fn paths_outside_the_members_checkouts_are_refused() {
    let f = fixture();
    let run = RunId::new();
    f.olga
        .send(Envelope::local(ClientCommand::SubmitPrompt { run, request: request("/Users/olga/rocket".into()) }))
        .expect("send");
    assert!(failure(next(&f.olga), run).contains("outside"));
    let escape = f.home.join("..").join("someone-else");
    f.olga.send(Envelope::local(ClientCommand::LoadMemory { path: escape })).expect("send");
    assert!(matches!(next(&f.olga), ServerEvent::Error { message } if message.contains("outside")));
    let read = ClientCommand::ReadLibraryItem { path: "/etc".into(), name: "passwd".into() };
    f.olga.send(Envelope::local(read)).expect("send");
    assert!(matches!(next(&f.olga), ServerEvent::Error { message } if message.contains("outside")));
}

#[test]
fn the_policy_binds_every_prompt() {
    let f = fixture();
    let policy = Policy { max_mode: Some(PermissionMode::Ask), ..Policy::default() };
    f.hosted.governance.set_policy(&policy).expect("policy");
    let run = RunId::new();
    let request = PromptRequest { permission: PermissionMode::Bypass, ..request(f.home.join("rocket")) };
    f.olga.send(Envelope::local(ClientCommand::SubmitPrompt { run, request })).expect("send");
    assert!(failure(next(&f.olga), run).contains("policy"));
}

#[test]
fn a_file_sent_from_the_members_computer_is_kept_among_their_checkouts() {
    let f = fixture();
    let upload =
        ClientCommand::Upload { from: "/Users/olga/sky.png".into(), name: "sky.png".into(), data: "c2t5".into() };
    f.olga.send(Envelope::local(upload)).expect("send");
    let ServerEvent::Uploaded { from, path } = next(&f.olga) else { panic!("not uploaded") };
    assert_eq!(from, PathBuf::from("/Users/olga/sky.png"));
    assert!(path.starts_with(&f.home), "{path:?}");
    assert_eq!(std::fs::read(&path).expect("read"), b"sky");
    let run = RunId::new();
    let mut request = request(f.home.join("rocket"));
    request.references.push(superfik_core::Reference { path });
    f.olga.send(Envelope::local(ClientCommand::SubmitPrompt { run, request })).expect("send");
    assert!(!failure(next(&f.olga), run).contains("outside"));
}

#[test]
fn a_member_signs_in_to_github_and_their_checkouts_reach_it_as_them() {
    let f = fixture();
    let locate =
        || ClientCommand::LocateRepository { slug: "acme/rocket".into(), known: vec![], clone_root: "/x".into() };
    f.olga.send(Envelope::local(locate())).expect("send");
    next(&f.olga);
    f.olga.send(Envelope::local(ClientCommand::SignInToGitHub)).expect("send");
    assert_eq!(
        next(&f.olga),
        ServerEvent::GitHubCode { code: "ABCD-1234".into(), url: "https://github.com/login/device".into() }
    );
    assert_eq!(next(&f.olga), ServerEvent::GitHubSignedIn { login: Some("olga-gh".into()) });
    f.olga.send(Envelope::local(locate())).expect("send");
    next(&f.olga);
    assert_eq!(*f.checkouts.tokens.lock().expect("lock"), [None, Some("ghu_olga".to_owned())]);
    f.olga.send(Envelope::local(ClientCommand::SignOutOfGitHub)).expect("send");
    assert_eq!(next(&f.olga), ServerEvent::GitHubSignedIn { login: None });
}

#[test]
fn tidying_memory_follows_the_policy_too() {
    let f = fixture();
    let policy = Policy { models: Some(vec!["claude-sonnet-5-5".into()]), ..Policy::default() };
    f.hosted.governance.set_policy(&policy).expect("policy");
    let run = RunId::new();
    let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
    f.olga
        .send(Envelope::local(ClientCommand::OptimizeMemory { run, path: f.home.join("rocket"), model }))
        .expect("send");
    assert!(failure(next(&f.olga), run).contains("policy"));
}

#[test]
fn a_routine_is_held_to_the_policy_when_it_is_written() {
    use superfik_core::routine::{Autonomy, RoutineDraft, RoutineEdit, Schedule};
    let f = fixture();
    let policy = Policy { max_mode: Some(PermissionMode::AcceptEdits), ..Policy::default() };
    f.hosted.governance.set_policy(&policy).expect("policy");
    let draft = |autonomy| RoutineDraft {
        name: "Upkeep".into(),
        brief: "Check Sentry".into(),
        schedule: Schedule::Every { minutes: 60 },
        autonomy,
        model: None,
        connections: vec![],
        answers: false,
    };
    let edit = |autonomy| ClientCommand::EditRoutines {
        path: f.home.join("rocket"),
        edits: vec![RoutineEdit::Create { draft: draft(autonomy) }],
    };
    f.olga.send(Envelope::local(edit(Autonomy::Auto))).expect("send");
    assert!(matches!(next(&f.olga), ServerEvent::Error { message } if message.contains("policy")));
    f.olga.send(Envelope::local(edit(Autonomy::Bounded))).expect("send");
    let ServerEvent::Routines { book, .. } = next(&f.olga) else { panic!("not written") };
    assert_eq!(book.routines.len(), 1);
}

#[test]
fn a_repository_is_located_in_the_servers_checkout() {
    let f = fixture();
    let locate = ClientCommand::LocateRepository {
        slug: "acme/rocket".into(),
        known: vec!["/Users/olga/rocket".into()],
        clone_root: "/Users/olga/Superfik".into(),
    };
    f.olga.send(Envelope::local(locate)).expect("send");
    assert_eq!(
        next(&f.olga),
        ServerEvent::RepositoryLocated { slug: "acme/rocket".into(), path: f.home.join("rocket") }
    );
    assert_eq!(*f.checkouts.prepared.lock().expect("lock"), [ProjectRef::Remote("github.com/acme/rocket".into())]);
}
