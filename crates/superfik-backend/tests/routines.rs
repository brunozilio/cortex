//! Routines end to end through the handler: a due routine fires as its
//! owner's run, and its report records the reply.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use superfik_backend::{CommandHandler, InMemoryStore, LocalServices, Memories, Outbox, Routines, Wake};
use superfik_core::routine::{Autonomy, Ending, RoutineBook, RoutineDraft, RoutineEdit, Schedule};
use superfik_core::{EngineEvent, EngineKind, ModelOption, Principal};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_workspace::{WorkspaceError, Worktrees};

const LATER: i64 = 4_000_000_000;

#[derive(Default)]
struct FakeWorktrees {
    added: Mutex<Vec<String>>,
    broken: bool,
    untouched: bool,
    removed: Mutex<Vec<String>>,
}

impl Worktrees for FakeWorktrees {
    fn add(&self, repo: &Path, path: &Path, branch: &str) -> Result<(), WorkspaceError> {
        if self.broken {
            return Err(WorkspaceError::InvalidBranch(branch.into()));
        }
        let entry = format!("{} {} {branch}", repo.display(), path.display());
        self.added.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(entry);
        Ok(())
    }

    fn remove_if_untouched(&self, _repo: &Path, _path: &Path, branch: &str) -> Result<bool, WorkspaceError> {
        if self.untouched {
            self.removed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(branch.to_owned());
        }
        Ok(self.untouched)
    }
}

fn handler(script: Vec<EngineEvent>) -> (CommandHandler, Receiver<ServerEvent>) {
    handler_with(script, Arc::new(FakeWorktrees::default()))
}

fn handler_with(script: Vec<EngineEvent>, worktrees: Arc<FakeWorktrees>) -> (CommandHandler, Receiver<ServerEvent>) {
    handler_on(EngineKind::Native, script, worktrees)
}

fn handler_on(
    model_engine: EngineKind,
    script: Vec<EngineEvent>,
    worktrees: Arc<FakeWorktrees>,
) -> (CommandHandler, Receiver<ServerEvent>) {
    let model = ModelOption::new(model_engine, "claude-opus-5-5", "Opus 5.5");
    let engine = FakeEngine::new(EngineKind::Native, vec![model], script);
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine)));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.routines = Routines::in_memory();
    services.worktrees = worktrees;
    services.worktree_root = "/data/worktrees".into();
    let (sender, events) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(|| {});
    (CommandHandler::new(services, Outbox::new(sender, wake)), events)
}

fn create(handler: &CommandHandler) {
    create_with(handler, Autonomy::Draft);
}

fn create_with(handler: &CommandHandler, autonomy: Autonomy) {
    let draft = RoutineDraft {
        name: "Upkeep".into(),
        brief: "Triage new Sentry issues".into(),
        schedule: Schedule::Every { minutes: 30 },
        autonomy,
        model: None,
        connections: Vec::new(),
        answers: false,
    };
    let path = Path::new("/repo").to_path_buf();
    let edits = vec![RoutineEdit::Create { draft }];
    handler.handle(Envelope::new(Principal::LocalOwner, ClientCommand::EditRoutines { path, edits }));
}

fn book_until(events: &Receiver<ServerEvent>, done: impl Fn(&RoutineBook) -> bool) -> RoutineBook {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(ServerEvent::Routines { book, .. }) = events.recv_timeout(Duration::from_millis(50))
            && done(&book)
        {
            return book;
        }
    }
    panic!("timed out waiting for a routine book");
}

#[test]
fn a_due_routine_fires_and_reports_its_reply() {
    let (handler, events) = handler(vec![
        EngineEvent::TextDelta("Nothing new in Sentry.".into()),
        EngineEvent::Completed { duration_ms: 10, cost_usd: Some(0.02) },
    ]);
    create(&handler);
    book_until(&events, |book| book.routines.len() == 1);

    handler.fire_due_routines(LATER);
    handler.fire_due_routines(LATER);

    let book = book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    let report = &book.reports[0];
    assert_eq!(book.reports.len(), 1);
    assert_eq!(report.ending, Some(Ending::Completed));
    assert_eq!(report.text, "Nothing new in Sentry.");
    assert_eq!(book.unread(), 1);
}

#[test]
fn a_routine_that_cannot_start_reports_why() {
    let (handler, events) = handler_on(EngineKind::Codex, vec![], Arc::new(FakeWorktrees::default()));
    create(&handler);
    book_until(&events, |book| book.routines.len() == 1);

    handler.fire_due_routines(LATER);

    let book = book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    assert!(matches!(&book.reports[0].ending, Some(Ending::Failed(why)) if why.contains("Codex")));
}

#[test]
fn a_routine_that_may_edit_works_in_a_worktree_on_its_own_branch() {
    let worktrees = Arc::new(FakeWorktrees::default());
    let (handler, events) =
        handler_with(vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }], Arc::clone(&worktrees));
    create_with(&handler, Autonomy::Bounded);
    book_until(&events, |book| book.routines.len() == 1);

    handler.fire_due_routines(LATER);

    let book = book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    let branch = book.reports[0].branch.clone().unwrap_or_else(|| panic!("no branch"));
    assert!(branch.starts_with("routine/upkeep-"));
    let added = worktrees.added.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    assert_eq!(added.len(), 1);
    assert!(added[0].starts_with("/repo /data/worktrees"), "{}", added[0]);
    assert!(added[0].ends_with(&branch));
    assert_eq!(book.reports[0].ending, Some(Ending::Completed));
}

#[test]
fn a_worktree_that_cannot_be_made_fails_the_firing_without_running_it() {
    let worktrees = Arc::new(FakeWorktrees { broken: true, ..FakeWorktrees::default() });
    let (handler, events) = handler_with(vec![EngineEvent::TextDelta("ran".into())], worktrees);
    create_with(&handler, Autonomy::Auto);
    book_until(&events, |book| book.routines.len() == 1);

    handler.fire_due_routines(LATER);

    let book = book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    assert!(matches!(&book.reports[0].ending, Some(Ending::Failed(why)) if why.contains("worktree")));
    assert_eq!(handler.active_runs(), 0);
}

#[test]
fn a_whatsapp_message_starts_the_routine_answering_its_account() {
    let (handler, events) = handler(vec![
        EngineEvent::TextDelta("Answered Ana.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]);
    let account = superfik_core::Connection {
        id: superfik_core::ConnectionId::new(),
        plugin: superfik_core::PluginId("whatsapp".into()),
        plugin_name: "WhatsApp".into(),
        label: "support".into(),
        icon: String::new(),
    };
    let draft = RoutineDraft {
        name: "Support".into(),
        brief: "Answer customers".into(),
        schedule: Schedule::Daily { minute: 18 * 60, utc_offset: 0 },
        autonomy: Autonomy::Draft,
        model: None,
        connections: vec![account.clone()],
        answers: true,
    };
    let path = Path::new("/repo").to_path_buf();
    let edits = vec![RoutineEdit::Create { draft }];
    handler.handle(Envelope::new(Principal::LocalOwner, ClientCommand::EditRoutines { path, edits }));
    book_until(&events, |book| book.routines.len() == 1);

    let message = superfik_core::channel::Message {
        id: "wamid-1".into(),
        exchange: "5511999990000".into(),
        is_group: false,
        sender: Some("5511999990000".into()),
        sender_name: Some("Ana".into()),
        text: "O checkout não abre".into(),
        media: None,
        mentioned_me: false,
        reply_to_me: false,
        at: superfik_backend::channels_now_ms(),
        source: superfik_core::channel::Source::Contact,
    };
    let event = superfik_channels::ChannelEvent::Message { message, delivery: superfik_core::channel::Delivery::Live };
    handler.channel_report(superfik_backend::Inbound { owner: Principal::LocalOwner, connection: account.id, event });

    let book = book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    assert_eq!(book.reports[0].exchange.as_deref(), Some("5511999990000"));
    assert_eq!(book.reports[0].text, "Answered Ana.");
    assert_eq!(book.reports[0].ending, Some(Ending::Completed));
}

#[test]
fn a_worktree_left_untouched_goes_and_its_report_names_no_branch() {
    let worktrees = Arc::new(FakeWorktrees { untouched: true, ..FakeWorktrees::default() });
    let (handler, events) =
        handler_with(vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }], Arc::clone(&worktrees));
    create_with(&handler, Autonomy::Bounded);
    book_until(&events, |book| book.routines.len() == 1);

    handler.fire_due_routines(LATER);

    let book = book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    assert_eq!(book.reports[0].branch, None);
    let removed = worktrees.removed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    assert_eq!(removed.len(), 1);
    assert!(removed[0].starts_with("routine/upkeep-"));
}
