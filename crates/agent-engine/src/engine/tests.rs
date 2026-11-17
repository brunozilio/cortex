use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use domain::{EngineProblem, FailureKind, RunId, SessionCommand, SessionId};

use super::stop::Stop;
use super::*;
use crate::boundary::EventSink;
use crate::checkpoints::Checkpoint;
use crate::model::TurnRequest;
use crate::model::scripted::ScriptedModel;
use crate::sessions::{Entry, InMemorySessions};
use crate::transcript::{Block, Message};

#[derive(Default)]
struct Heard(Mutex<Vec<EngineEvent>>);

impl EventSink for Heard {
    fn emit(&self, _run: RunId, event: EngineEvent) {
        self.0.lock().expect("events").push(event);
    }
}

impl Heard {
    fn events(&self) -> Vec<EngineEvent> {
        self.0.lock().expect("events").clone()
    }
}

struct Account(Option<Arc<ScriptedModel>>);

impl ApiSource for Account {
    fn api(&self, _request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        match &self.0 {
            Some(model) => Ok(Arc::clone(model) as Arc<dyn ModelApi>),
            None => Err(ModelError::Unauthorized("no account connected".into())),
        }
    }
}

#[derive(Default)]
struct Store {
    kept: InMemorySessions,
    writes: AtomicUsize,
}

impl Store {
    fn written(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    fn wrote(&self) {
        self.writes.fetch_add(1, Ordering::SeqCst);
    }
}

impl Sessions for Store {
    fn load(&self, id: &SessionId) -> Option<Vec<Message>> {
        self.kept.load(id)
    }

    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.wrote();
        self.kept.save(id, history)
    }

    fn record(&self, _cwd: &std::path::Path, _id: &SessionId, _entry: &Entry) -> Result<(), String> {
        self.wrote();
        Ok(())
    }

    fn save_checkpoints(&self, _cwd: &std::path::Path, _id: &SessionId, _all: &[Checkpoint]) -> Result<(), String> {
        self.wrote();
        Ok(())
    }

    fn save_snapshot(
        &self,
        _cwd: &std::path::Path,
        _id: &SessionId,
        _snapshot: &crate::prompt::Snapshot,
    ) -> Result<(), String> {
        self.wrote();
        Ok(())
    }
}

struct Staged {
    job: Job,
    heard: Arc<Heard>,
    store: Arc<Store>,
    _dir: tempfile::TempDir,
}

fn staged(model: Option<Arc<ScriptedModel>>, edit: impl FnOnce(&mut PromptRequest)) -> Staged {
    let dir = tempfile::tempdir().expect("dir");
    let mut request = PromptRequest::plain(dir.path().into(), "go", ModelOption::new("m", "M"));
    let home = dir.path().join("home");
    request.host.extensions = Some(domain::Extensions { home: Some(home), ..Default::default() });
    edit(&mut request);
    let (heard, store) = (Arc::new(Heard::default()), Arc::new(Store::default()));
    let workspace = Workspace::for_request(&request);
    let job = Job {
        source: Arc::new(Account(model)),
        sessions: Arc::clone(&store) as Arc<dyn Sessions>,
        mcp: Arc::new(NetHttp),
        lsp: Arc::new(LspPool::default()),
        control: Arc::new(Control::new(request.policy.permission, workspace.stop.clone())),
        gate: RunGate::new(RunId::new(), Arc::clone(&heard) as Arc<dyn EventSink>),
        request,
        recorder: None,
        held: Arc::default(),
        live: super::live::Live::of(&Arc::default()),
    };
    Staged { job, heard, store, _dir: dir }
}

fn said(text: &str) -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text(text.into())])]))
}

fn failure(stop: Result<impl Sized, Stop>) -> String {
    match stop {
        Err(Stop::Failed { message, kind: FailureKind::Other }) => message,
        Err(other) => panic!("stopped as {other:?}"),
        Ok(_) => panic!("the phase went on"),
    }
}

#[test]
fn a_session_command_with_no_conversation_stops_before_anything_is_loaded() {
    let mut staged = staged(None, |request| request.session.command = Some(SessionCommand::Compact));
    assert_eq!(failure(staged.job.load_session()), "There is no conversation to work on yet.");
    assert!(staged.job.recorder.is_none());
    assert_eq!(staged.store.written(), 0);
}

#[test]
fn a_conversation_that_is_not_there_is_diagnosed() {
    let mut staged = staged(None, |request| request.session.resume = Some(SessionId("gone".into())));
    let stop = staged.job.load_session().err().expect("stopped");
    assert!(
        matches!(&stop, Stop::Diagnosed { problem: EngineProblem::SessionNotFound, message } if message == "The conversation to continue was not found."),
        "{stop:?}"
    );
}

#[test]
fn a_first_prompt_gets_a_new_conversation_that_is_not_announced_yet() {
    let mut staged = staged(None, |_| {});
    let session = staged.job.load_session().expect("loaded");
    assert!(session.history.is_empty());
    assert!(uuid::Uuid::parse_str(&session.id.0).is_ok(), "{}", session.id.0);
    assert_eq!(staged.heard.events(), []);
    assert_eq!(staged.store.written(), 0, "nothing is written until the model is reachable");
}

fn kept(staged: &Staged, id: &str, history: &[Message]) -> SessionId {
    let id = SessionId(id.into());
    staged.store.kept.save_for(&staged.job.request.session.cwd, &id, history).expect("saved");
    id
}

#[test]
fn a_resumed_conversation_keeps_its_id_and_is_announced_at_once() {
    let mut staged = staged(None, |request| request.session.resume = Some(SessionId("kept".into())));
    let id = kept(&staged, "kept", &[Message::user("before")]);
    let session = staged.job.load_session().expect("loaded");
    assert_eq!((&session.id, session.history.len()), (&id, 1));
    assert_eq!(staged.heard.events(), [EngineEvent::SessionStarted(id)]);
}

#[test]
fn a_fork_carries_the_history_into_a_new_conversation() {
    let mut staged = staged(None, |request| {
        request.session.resume = Some(SessionId("kept".into()));
        request.session.fork = true;
    });
    let id = kept(&staged, "kept", &[Message::user("before")]);
    let session = staged.job.load_session().expect("loaded");
    assert_ne!(session.id, id);
    assert_eq!(session.history.len(), 1);
    assert_eq!(staged.heard.events(), [], "a fork is announced when the model is reachable");
}

#[test]
fn a_chore_ends_the_run_from_load_session_and_opens_no_conversation() {
    let mut staged = staged(Some(said("A title")), |request| {
        request.session.command = Some(SessionCommand::Title);
        request.session.resume = Some(SessionId("gone".into()));
    });
    let stop = staged.job.load_session().err().expect("stopped");
    assert!(matches!(stop, Stop::Completed { .. }), "{stop:?}");
    assert!(staged.job.recorder.is_none());
    assert_eq!(staged.store.written(), 0);
}

#[test]
fn a_chore_without_an_account_stops_on_the_model_s_error() {
    let mut staged = staged(None, |request| request.session.command = Some(SessionCommand::Title));
    let stop = staged.job.load_session().err().expect("stopped");
    assert!(matches!(stop, Stop::Model(ModelError::Unauthorized(_))), "{stop:?}");
}

#[test]
fn a_rewind_to_a_checkpoint_that_was_never_taken_stops_from_load_session() {
    let mut staged = staged(None, |request| {
        request.session.command = Some(SessionCommand::Rewind(3));
        request.session.resume = Some(SessionId("kept".into()));
    });
    kept(&staged, "kept", &[Message::user("before")]);
    assert!(failure(staged.job.load_session()).starts_with("Could not rewind: "));
}

#[test]
fn a_branch_from_a_checkpoint_that_was_never_taken_stops_from_load_session() {
    let mut staged = staged(None, |request| {
        request.session.command = Some(SessionCommand::Branch(3));
        request.session.resume = Some(SessionId("kept".into()));
    });
    kept(&staged, "kept", &[Message::user("before")]);
    assert!(failure(staged.job.load_session()).starts_with("Could not branch: "));
    assert_eq!(staged.heard.events(), []);
}

#[test]
fn a_run_without_an_account_stops_in_open_and_creates_no_session() {
    let mut staged = staged(None, |_| {});
    let workspace = Workspace::for_request(&staged.job.request);
    let stop = staged.job.phases(workspace).expect_err("stopped");
    assert!(matches!(stop, Stop::Model(ModelError::Unauthorized(_))), "{stop:?}");
    assert_eq!(staged.store.written(), 0);
    assert_eq!(staged.heard.events(), []);
}

#[test]
fn measuring_the_context_needs_no_account() {
    let mut staged = staged(None, |request| {
        request.session.command = Some(SessionCommand::MeasureContext);
        request.session.resume = Some(SessionId("kept".into()));
    });
    kept(&staged, "kept", &[Message::user("before")]);
    let workspace = Workspace::for_request(&staged.job.request);
    let stop = staged.job.phases(workspace).expect_err("stopped");
    assert!(matches!(stop, Stop::Completed { cost_usd: Some(cost) } if cost == 0.0), "{stop:?}");
    let events = staged.heard.events();
    assert!(matches!(events.last(), Some(EngineEvent::ContextMeasured(_))), "{events:?}");
}

fn ended(stop: Stop, cancelled: bool) -> Vec<EngineEvent> {
    let staged = staged(None, |_| {});
    if cancelled {
        staged.job.control.cancel();
    }
    staged.job.end(stop, Instant::now());
    staged.heard.events()
}

#[test]
fn every_stop_becomes_its_events_in_one_place() {
    let failed = |kind, message: &str| vec![EngineEvent::Failure(kind), EngineEvent::Failed(message.into())];
    assert_eq!(ended(Stop::failed("no"), false), failed(FailureKind::Other, "no"));
    assert_eq!(ended(Stop::Cancelled, false), [EngineEvent::Cancelled]);
    let diagnosed = Stop::Diagnosed { problem: EngineProblem::SessionNotFound, message: "gone".into() };
    let mut expected = vec![EngineEvent::Diagnosed(EngineProblem::SessionNotFound)];
    expected.extend(failed(FailureKind::Other, "gone"));
    assert_eq!(ended(diagnosed, false), expected);
    let overloaded = ModelError::Overloaded("529".into());
    let message = overloaded.to_string();
    assert_eq!(ended(Stop::Model(overloaded), false), failed(FailureKind::Unavailable, &message));
    let signed_out = ModelError::Unauthorized("no account".into());
    let mut expected = vec![EngineEvent::Diagnosed(EngineProblem::NotLoggedIn)];
    expected.extend(failed(FailureKind::Other, &signed_out.to_string()));
    assert_eq!(ended(Stop::Model(signed_out), false), expected);
    assert_eq!(ended(Stop::Model(ModelError::Cancelled), false), [EngineEvent::Cancelled]);
    let completed = ended(Stop::completed(Some(0.5)), false);
    assert!(matches!(completed[..], [EngineEvent::Completed { cost_usd: Some(cost), .. }] if cost == 0.5));
}

#[test]
fn a_cancelled_run_ends_as_cancelled_however_it_stopped() {
    let signed_out = || Stop::Model(ModelError::Unauthorized("no account".into()));
    for stop in [Stop::failed("no"), Stop::completed(None), signed_out()] {
        assert_eq!(ended(stop, true), [EngineEvent::Cancelled]);
    }
}

#[test]
fn a_pick_of_the_small_model_switches_the_run_to_it_and_forgets_the_old_model_s_limits() {
    let judge = said(r#"{"tier":"small","effort":"low"}"#);
    let mut staged = staged(None, |request| request.prompt.text = "rename x to y".into());
    let mut models = crate::tools::Models::new(staged.job.request.session.model.clone());
    models.small_model = Some("m".into());
    models.model.id = "large".into();
    models.model.window = Some(200_000);
    staged.job.request.session.model.id = "large".into();
    let mut template = TurnRequest {
        model: "large".into(),
        system: String::new(),
        messages: Vec::new(),
        tools: Vec::new(),
        effort: domain::Effort::High,
        max_tokens: 100,
    };
    let api = judge as Arc<dyn ModelApi>;
    staged.job.auto_pick(&api, &Arc::default(), &mut template, &mut models);
    assert_eq!((models.model.id.as_str(), models.model.window), ("m", None));
    assert_eq!((template.model.as_str(), template.effort), ("m", domain::Effort::Low));
    assert_eq!(staged.job.request.session.model.id, "m");
    assert_eq!(
        staged.heard.events().last(),
        Some(&EngineEvent::Picked { model: "m".into(), effort: domain::Effort::Low })
    );
}
