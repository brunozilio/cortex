//! Reducer rules for the context window: measuring after replies,
//! compacting on request, and keeping both out of the transcript.

use std::path::PathBuf;

use superfik_core::{
    Compaction, ContextUsage, EngineEvent, EngineKind, ModelOption, PromptRequest, RunId, SessionCommand, SessionId,
};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::tests::commands;
use super::*;

fn session() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.models.push(ModelOption::new(EngineKind::Codex, "gpt", "GPT"));
    s
}

fn submits(effects: &[Effect]) -> Vec<(RunId, PromptRequest)> {
    commands(effects)
        .into_iter()
        .filter_map(|c| match c {
            ClientCommand::SubmitPrompt { run, request } => Some((*run, request.clone())),
            _ => None,
        })
        .collect()
}

fn event(s: &mut Session, run: RunId, event: EngineEvent) -> Vec<Effect> {
    s.apply(Action::Server(ServerEvent::Run { run, event }), 2.0)
}

fn completed() -> EngineEvent {
    EngineEvent::Completed { duration_ms: 10, cost_usd: None }
}

fn usage(used: u64) -> ContextUsage {
    ContextUsage { used, window: 1_000_000, reserved: 33_000, shares: vec![] }
}

fn reply(s: &mut Session) -> (RunId, PromptRequest) {
    s.apply(Action::Edit(EditKey::Insert("hello".into())), 0.0);
    let (run, _) = submits(&s.apply(Action::Submit, 1.0))[0].clone();
    event(s, run, EngineEvent::SessionStarted(SessionId("abc".into())));
    event(s, run, EngineEvent::TextDelta("Hi".into()));
    let measure: Vec<_> = submits(&event(s, run, completed()))
        .into_iter()
        .filter(|(_, request)| request.command == Some(SessionCommand::MeasureContext))
        .collect();
    assert_eq!(measure.len(), 1, "a completed reply measures the context");
    measure[0].clone()
}

#[test]
fn a_completed_reply_measures_the_context_off_the_transcript() {
    let mut s = session();
    let (run, request) = reply(&mut s);
    assert_eq!(request.command, Some(SessionCommand::MeasureContext));
    assert_eq!(request.resume, Some(SessionId("abc".into())));
    assert!(s.conversation().is_measuring() && !s.is_running(), "measuring does not hold the composer");
    event(&mut s, run, EngineEvent::TextDelta("## Context Usage".into()));
    event(&mut s, run, EngineEvent::ContextMeasured(usage(19_800)));
    event(&mut s, run, completed());
    let c = s.conversation();
    assert_eq!(c.context, Some(usage(19_800)));
    assert_eq!(c.upkeep, None);
    assert_eq!(c.turns.len(), 1);
    assert_eq!(c.turns[0].response.text, "Hi", "the report never reaches the reply");
}

#[test]
fn a_failed_reply_or_one_without_a_session_is_not_measured() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("hello".into())), 0.0);
    let (run, _) = submits(&s.apply(Action::Submit, 1.0))[0].clone();
    assert!(submits(&event(&mut s, run, completed())).is_empty());
    assert!(!s.conversation().can_compact());
    assert!(s.apply(Action::CompactContext, 3.0).iter().all(|e| matches!(e, Effect::Sound(_))));
}

#[test]
fn a_prompt_sent_while_measuring_stops_the_measurement() {
    let mut s = session();
    let (measure, _) = reply(&mut s);
    s.apply(Action::Edit(EditKey::Insert("next".into())), 3.0);
    let effects = s.apply(Action::Submit, 3.0);
    let sent = commands(&effects);
    assert!(matches!(sent[0], ClientCommand::CancelRun { run } if *run == measure));
    assert!(matches!(sent[1], ClientCommand::SubmitPrompt { .. }));
    assert_eq!(s.conversation().upkeep, None);
    event(&mut s, measure, EngineEvent::ContextMeasured(usage(1)));
    assert_eq!(s.conversation().context, None);
}

#[test]
fn compacting_holds_prompts_then_measures_again() {
    let mut s = session();
    let (measure, _) = reply(&mut s);
    event(&mut s, measure, EngineEvent::ContextMeasured(usage(150_000)));
    event(&mut s, measure, completed());
    let (compact, request) = submits(&s.apply(Action::CompactContext, 3.0))[0].clone();
    assert_eq!(request.command, Some(SessionCommand::Compact));
    assert!(s.conversation().is_compacting() && !s.conversation().can_compact());

    s.apply(Action::Edit(EditKey::Insert("wait".into())), 3.0);
    assert!(submits(&s.apply(Action::Submit, 3.0)).is_empty(), "prompts wait for the compaction");

    let freed = Compaction { before: 150_000, after: 20_000 };
    event(&mut s, compact, EngineEvent::Compacted(freed));
    let again = submits(&event(&mut s, compact, completed()));
    assert_eq!(again[0].1.command, Some(SessionCommand::MeasureContext));
    assert_eq!(s.conversation().compaction, Some(freed));
    assert_eq!(s.conversation().turns.len(), 1);
}

#[test]
fn the_compact_command_shows_the_context_node_and_compacts() {
    let mut s = session();
    let (measure, _) = reply(&mut s);
    event(&mut s, measure, EngineEvent::ContextMeasured(usage(150_000)));
    event(&mut s, measure, completed());
    assert!(!s.nodes.context, "hidden until asked for");
    s.apply(Action::Edit(EditKey::Insert("/compact".into())), 3.0);
    let effects = s.apply(Action::Submit, 3.0);
    assert!(s.nodes.context, "the node that shows the compaction opens");
    assert_eq!(submits(&effects)[0].1.command, Some(SessionCommand::Compact));
    assert!(s.conversation().is_compacting());
}

#[test]
fn a_failed_compaction_says_so_and_a_failed_measurement_does_not() {
    let mut s = session();
    let (measure, _) = reply(&mut s);
    event(&mut s, measure, EngineEvent::Failed("no".into()));
    assert_eq!(s.notice, None);
    let (compact, _) = submits(&s.apply(Action::CompactContext, 3.0))[0].clone();
    event(&mut s, compact, EngineEvent::Failed("Compaction failed".into()));
    assert_eq!(s.notice, Some(Notice::Raw("Compaction failed".into())));
    assert!(s.conversation().can_compact());
}

#[test]
fn another_model_remeasures_and_another_engine_forgets() {
    let mut s = session();
    let (measure, _) = reply(&mut s);
    event(&mut s, measure, EngineEvent::ContextMeasured(usage(19_800)));
    event(&mut s, measure, completed());
    let again = submits(&s.apply(Action::SelectModel(3), 3.0));
    assert_eq!(again[0].1.model.label, "Haiku 5.5");
    assert_eq!(again[0].1.command, Some(SessionCommand::MeasureContext));
    s.conversation_mut().upkeep = None;
    s.engines = vec![];
    let codex = s.models.len() - 1;
    s.apply(Action::SelectModel(codex), 4.0);
    if s.conversation().model == codex {
        assert_eq!((s.conversation().context.clone(), s.conversation().engine_session.clone()), (None, None));
    }
}

#[test]
fn compacting_has_a_sound_and_so_does_its_end() {
    let mut s = session();
    let (measure, _) = reply(&mut s);
    event(&mut s, measure, completed());
    let effects = s.apply(Action::CompactContext, 3.0);
    assert!(effects.iter().any(|e| matches!(e, Effect::Sound(superfik_sound::Cue::Button))));
    let (compact, _) = submits(&effects)[0].clone();
    let done = event(&mut s, compact, completed());
    assert!(done.iter().any(|e| matches!(e, Effect::Sound(superfik_sound::Cue::Notification))));
}
