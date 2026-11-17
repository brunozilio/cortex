use crate::api::features::memory::{MemoryOp, MemoryRequest};
use crate::api::{FeatureError, FeatureRequest, ServerEvent};
use domain::{ModelOption, PromptRequest};

use super::*;
use crate::client::testing::{Quiet, scripted, until, within_five_seconds};

fn prompt(run: RunId) -> ClientCommand {
    let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
    ClientCommand::SubmitPrompt { run, request }
}

#[test]
fn a_run_whose_backend_went_away_ends_with_an_error_instead_of_hanging() {
    let (wake, woken) = crossbeam_channel::bounded::<()>(1);
    let client = Arc::new(Dispatcher::new(Box::new(Quiet), woken));
    let mut run = Run::start(&client, prompt).expect("started");

    drop(wake);

    let (ended, mut run) = within_five_seconds(move || (run.next_event(), run));
    assert_eq!(ended, Some(EngineEvent::Failed(BACKEND_GONE.into())));
    assert!(run.is_done() && run.next_event().is_none(), "it ended once");
    let mut later = Run::start(&client, prompt);
    let refused = match &mut later {
        Ok(run) => run.next_event(),
        Err(_) => None,
    };
    assert!(later.is_err() || refused == Some(EngineEvent::Failed(BACKEND_GONE.into())), "{later:?}");
}

#[test]
fn a_run_whose_backend_went_away_reads_as_failed_to_its_text() {
    let (wake, woken) = crossbeam_channel::bounded::<()>(1);
    let client = Arc::new(Dispatcher::new(Box::new(Quiet), woken));
    let run = Run::start(&client, prompt).expect("started");
    drop(wake);
    let text = within_five_seconds(move || run.text());
    assert!(matches!(&text, Err(Error::Run(message)) if message == BACKEND_GONE), "{text:?}");
}

#[test]
fn a_run_the_backend_refused_fails_instead_of_waiting_for_events() {
    let (client, _, _) = scripted(|command, outlet| match &command.payload {
        ClientCommand::SubmitPrompt { .. } => outlet.reply(command.id, Reply::error("that run is already in progress")),
        ClientCommand::Feature(request) => {
            let refused = FeatureError::Failed { feature: request.feature(), message: "no such model".into() };
            outlet.reply(command.id, Reply::FeatureFailed(refused));
        }
        _ => {}
    });
    let mut refused = Run::start(&client, prompt).expect("sent");
    let mut unstarted = Run::start(&client, |run| {
        let op = MemoryOp::Optimize { run, model: ModelOption::new("m", "M") };
        ClientCommand::Feature(FeatureRequest::Memory(MemoryRequest { path: "/repo".into(), op }))
    })
    .expect("sent");

    let (first, second) = within_five_seconds(move || (refused.next_event(), unstarted.next_event()));

    assert_eq!(first, Some(EngineEvent::Failed("that run is already in progress".into())));
    assert_eq!(second, Some(EngineEvent::Failed("no such model".into())));
}

#[test]
fn a_run_that_was_accepted_streams_its_events_to_the_end() {
    let (client, _, _) = scripted(|command, outlet| {
        if let ClientCommand::SubmitPrompt { run, .. } = &command.payload {
            outlet.reply(command.id, Reply::Done);
            outlet.emit(ServerEvent::Run { run: *run, event: EngineEvent::TextDelta("Hello".into()) });
            outlet
                .emit(ServerEvent::Run { run: *run, event: EngineEvent::Completed { duration_ms: 1, cost_usd: None } });
        }
    });
    let run = Run::start(&client, prompt).expect("started");
    let text = within_five_seconds(move || run.text());
    assert!(matches!(&text, Ok(text) if text == "Hello"), "{text:?}");
}

fn cancelled(asked: &crate::client::testing::Asked, run: RunId) -> bool {
    asked.locked().iter().any(|command| command.payload == ClientCommand::CancelRun { run })
}

#[test]
fn dropping_a_run_that_has_not_ended_cancels_it() {
    let (client, _, asked) = scripted(|_, _| {});
    let run = Run::start(&client, prompt).expect("started");
    let id = run.id();
    drop(run);
    until(|| cancelled(&asked, id).then_some(()));
}

#[test]
fn a_run_left_running_or_already_ended_is_not_cancelled_when_dropped() {
    let (client, _, asked) = scripted(|command, outlet| {
        if let ClientCommand::SubmitPrompt { run, request } = &command.payload
            && request.prompt.text == "hi"
        {
            outlet.emit(ServerEvent::Run { run: *run, event: EngineEvent::Cancelled });
        }
    });
    let mut ended = Run::start(&client, prompt).expect("started");
    let ended_id = ended.id();
    let ended = within_five_seconds(move || {
        let last = ended.next_event();
        drop(ended);
        last
    });
    assert_eq!(ended, Some(EngineEvent::Cancelled));
    let left = Run::start(&client, |run| {
        let request = PromptRequest::plain("/repo".into(), "keep going", ModelOption::new("m", "M"));
        ClientCommand::SubmitPrompt { run, request }
    })
    .expect("started");
    let left_id = left.id();
    left.leave_running();
    client.send(ClientCommand::ListModels).expect("sent");
    until(|| asked.locked().iter().any(|command| command.payload == ClientCommand::ListModels).then_some(()));
    assert!(!cancelled(&asked, ended_id) && !cancelled(&asked, left_id), "{:?}", asked.locked());
}
