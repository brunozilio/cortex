use agent_engine::fake::FakeEngine;
use domain::SessionId;
use sdk::{EngineEvent, Model};

use super::app::{App, Msg};
use super::tests::{screen, sdk, until};

fn script() -> Vec<EngineEvent> {
    vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("Done.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]
}

fn prompts(engine: &FakeEngine) -> Vec<String> {
    engine.requests().into_iter().filter(|request| request.command.is_none()).map(|request| request.prompt).collect()
}

#[test]
fn events_that_reached_the_inbox_are_sent_as_a_turn_when_idle() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script());
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    let notice = domain::inbox::Notice::new("monitor bash_4", "ERROR: disk full");
    app.message(Msg::Notices(vec![notice]));
    until(&mut app, |app| !app.busy() && !prompts(&engine).is_empty());
    let sent = &prompts(&engine)[0];
    assert!(sent.contains("<event source=\"monitor bash_4\">") && sent.contains("ERROR: disk full"), "{sent}");
    assert!(screen(&app).contains("[monitor bash_4] ERROR: disk full"), "{}", screen(&app));
}
