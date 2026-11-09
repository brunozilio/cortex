//! A routine firing answers to the server's budget: refused before it
//! starts when the budget says so, its cost counted when it ends.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use superfik_backend::{Budget, CommandHandler, InMemoryStore, LocalServices, Memories, Outbox, Routines, Wake};
use superfik_core::routine::{Autonomy, Ending, RoutineBook, RoutineDraft, RoutineEdit, Schedule};
use superfik_core::{EngineEvent, EngineKind, ModelOption, Principal, PromptRequest};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};

const LATER: i64 = 4_000_000_000;

#[derive(Default)]
struct FakeBudget {
    spent: bool,
    counted: Mutex<Vec<(String, f64)>>,
}

impl Budget for FakeBudget {
    fn admit(&self, _owner: &Principal, _request: &PromptRequest) -> Result<(), String> {
        if self.spent { Err("the monthly budget is spent".into()) } else { Ok(()) }
    }

    fn spend(&self, _owner: &Principal, model: &str, cost_usd: f64) {
        self.counted.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((model.to_owned(), cost_usd));
    }
}

fn handler(script: Vec<EngineEvent>, budget: Arc<FakeBudget>) -> (CommandHandler, Receiver<ServerEvent>) {
    let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
    let engine = FakeEngine::new(EngineKind::Native, vec![model], script);
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine)));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.routines = Routines::in_memory();
    services.budget = budget;
    let (sender, events) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(|| {});
    let handler = CommandHandler::new(services, Outbox::new(sender, wake));
    let draft = RoutineDraft {
        name: "Upkeep".into(),
        brief: "Triage new Sentry issues".into(),
        schedule: Schedule::Every { minutes: 30 },
        autonomy: Autonomy::Draft,
        model: None,
        connections: Vec::new(),
        answers: false,
    };
    let edits = vec![RoutineEdit::Create { draft }];
    let path = Path::new("/repo").to_path_buf();
    handler.handle(Envelope::new(Principal::LocalOwner, ClientCommand::EditRoutines { path, edits }));
    book_until(&events, |book| book.routines.len() == 1);
    (handler, events)
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
fn a_firing_over_budget_is_reported_and_never_runs() {
    let budget = Arc::new(FakeBudget { spent: true, ..FakeBudget::default() });
    let (handler, events) = handler(vec![EngineEvent::TextDelta("ran".into())], budget);

    handler.fire_due_routines(LATER);

    let book = book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    assert!(matches!(&book.reports[0].ending, Some(Ending::Failed(why)) if why.contains("budget")));
    assert_eq!(book.reports[0].text, "");
    assert_eq!(handler.active_runs(), 0);
}

#[test]
fn a_finished_firings_cost_is_counted() {
    let budget = Arc::new(FakeBudget::default());
    let script = vec![EngineEvent::Completed { duration_ms: 5, cost_usd: Some(0.42) }];
    let (handler, events) = handler(script, Arc::clone(&budget));

    handler.fire_due_routines(LATER);

    book_until(&events, |book| book.reports.first().is_some_and(|r| !r.is_running()));
    let counted = budget.counted.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    assert_eq!(counted, [("claude-opus-5-5".to_owned(), 0.42)]);
}
