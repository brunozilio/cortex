//! A scheduled routine run on the native engine, accounted once.
use super::*;

#[test]
fn scheduled_native_run_finishes_report_and_is_accounted_once_on_server() {
    use std::sync::Mutex;
    use superfik_backend::{Budget, CommandHandler, Outbox, Routines};
    use superfik_core::routine::{Autonomy, Ending, RoutineDraft, RoutineEdit, Schedule};
    struct RoutineSource;
    impl ApiSource for RoutineSource {
        fn api(&self, _: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
            Ok(Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text(
                "Native routine completed.".into(),
            )])])))
        }
    }
    #[derive(Default)]
    struct CountBudget(Mutex<Vec<f64>>);
    impl Budget for CountBudget {
        fn authoritative(&self) -> bool {
            true
        }
        fn admit(&self, _: &Principal, _: &PromptRequest) -> Result<(), String> {
            Ok(())
        }
        fn spend(&self, _: &Principal, _: &str, cost: f64) {
            self.0.lock().expect("valid test fixture").push(cost);
        }
    }
    let dir = tempfile::tempdir().expect("valid test fixture");
    let engine = NativeEngine::new(Arc::new(RoutineSource), Arc::new(InMemorySessions::default()));
    let mut services =
        LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine))).kept_in(&dir.path().join("data"));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.routines = Routines::in_memory();
    services.plugins = services.plugins.with_secrets(Arc::new(superfik_backend::InMemorySecretStore::default()));
    services.providers = services.providers.with_store(Arc::new(superfik_backend::InMemoryProviderStore::default()));
    let budget = Arc::new(CountBudget::default());
    services.budget = budget.clone();
    let (sender, events) = crossbeam_channel::unbounded();
    let handler = CommandHandler::new(services, Outbox::new(sender, Arc::new(|| {})));
    let draft = RoutineDraft {
        name: "Native audit".into(),
        brief: "Summarize current work.".into(),
        schedule: Schedule::Every { minutes: 30 },
        autonomy: Autonomy::Draft,
        model: Some("anthropic:claude-sonnet-5-5".into()),
        connections: vec![],
        answers: false,
    };
    handler.handle(Envelope::local(ClientCommand::EditRoutines {
        path: dir.path().into(),
        edits: vec![RoutineEdit::Create { draft }],
    }));
    handler.fire_due_routines(4_000_000_000);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut report = None;
    let mut accounted = false;
    while Instant::now() < deadline {
        if let Ok(event) = events.recv_timeout(Duration::from_millis(25)) {
            match event {
                ServerEvent::Run { event: EngineEvent::Usage(usage), .. } => accounted |= usage.accounted_by_server,
                ServerEvent::Routines { book, .. } => {
                    if let Some(done) = book.reports.into_iter().find(|r| !r.is_running()) {
                        report = Some(done);
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    let report = report.expect("routine did not finish");
    assert_eq!(report.ending, Some(Ending::Completed));
    assert!(report.text.contains("Native routine completed"));
    let counted = budget.0.lock().expect("valid test fixture");
    assert_eq!(counted.len(), 1, "routine must not count Usage and Completed twice");
    assert!(counted[0] > 0.0);
    assert!(accounted);
}
