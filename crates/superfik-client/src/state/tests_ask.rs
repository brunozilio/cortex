//! Asks: the agent pauses, the user answers, the run resumes.

use std::path::PathBuf;

use superfik_core::{AgentAsk, AskId, AskKind, Choice, Decision, EngineEvent, PermissionMode, Question, RunId};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::tests::commands;
use super::*;

fn running() -> (Session, RunId) {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.apply(Action::Edit(EditKey::Insert("Ship it".into())), 0.0);
    let effects = s.apply(Action::Submit, 1.0);
    let run = match commands(&effects).as_slice() {
        [ClientCommand::SubmitPrompt { run, .. }] => *run,
        other => panic!("unexpected {other:?}"),
    };
    (s, run)
}

fn event(s: &mut Session, run: RunId, event: EngineEvent) -> Vec<Effect> {
    s.apply(Action::Server(ServerEvent::Run { run, event }), 2.0)
}

fn permission(id: &str, rememberable: bool) -> EngineEvent {
    let kind = AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable };
    EngineEvent::Asked(AgentAsk { id: AskId(id.into()), kind })
}

fn questions() -> EngineEvent {
    let options = |labels: &[&str]| {
        labels.iter().map(|l| Choice { label: (*l).into(), description: String::new() }).collect::<Vec<_>>()
    };
    let q = |text: &str, labels: &[&str], multi: bool| Question {
        header: "H".into(),
        text: text.into(),
        options: options(labels),
        multi,
    };
    let kind = AskKind::Questions(vec![q("Which?", &["a", "b", "c"], false), q("Extras?", &["x", "y", "z"], true)]);
    EngineEvent::Asked(AgentAsk { id: AskId("q".into()), kind })
}

fn answer(effects: &[Effect]) -> Option<(RunId, AskId, Decision)> {
    commands(effects).into_iter().find_map(|c| match c {
        ClientCommand::AnswerAsk { run, ask, decision } => Some((*run, ask.clone(), decision.clone())),
        _ => None,
    })
}

#[test]
fn asks_queue_and_are_answered_oldest_first() {
    let (mut s, run) = running();
    event(&mut s, run, permission("a", true));
    event(&mut s, run, permission("b", false));
    event(&mut s, run, permission("a", true));
    assert_eq!(s.conversation().asks.len(), 2, "a repeated ask is not queued twice");
    assert!(s.conversation().is_waiting());

    let first = s.apply(Action::Reply(Reply::Allow), 3.0);
    assert_eq!(answer(&first), Some((run, AskId("a".into()), Decision::Allow)));
    let second = s.apply(Action::Reply(Reply::AllowAlways), 3.0);
    assert_eq!(answer(&second), Some((run, AskId("b".into()), Decision::Allow)), "nothing to remember");
    assert!(!s.conversation().is_waiting());
    assert!(answer(&s.apply(Action::Reply(Reply::Allow), 3.0)).is_none());
}

#[test]
fn always_and_deny_reach_the_engine() {
    let (mut s, run) = running();
    event(&mut s, run, permission("a", true));
    assert_eq!(answer(&s.apply(Action::Reply(Reply::AllowAlways), 3.0)).map(|a| a.2), Some(Decision::AllowAlways));
    event(&mut s, run, permission("b", true));
    assert_eq!(answer(&s.apply(Action::Reply(Reply::Deny), 3.0)).map(|a| a.2), Some(Decision::Deny));
}

#[test]
fn digits_pick_a_permission_reply_in_display_order() {
    let (mut s, run) = running();
    event(&mut s, run, permission("a", true));
    assert!(answer(&s.apply(Action::PickChoice(7), 3.0)).is_none(), "no such row");
    assert_eq!(answer(&s.apply(Action::PickChoice(2), 3.0)).map(|a| a.2), Some(Decision::Deny));
    event(&mut s, run, permission("b", false));
    assert_eq!(answer(&s.apply(Action::PickChoice(1), 3.0)).map(|a| a.2), Some(Decision::Deny));
}

#[test]
fn questions_collect_picks_then_answer_once() {
    let (mut s, run) = running();
    event(&mut s, run, questions());
    assert!(answer(&s.apply(Action::PickChoice(1), 3.0)).is_none(), "a single pick moves to the next question");
    assert_eq!(s.pending_ask().map(|a| a.question), Some(1));
    s.apply(Action::PickChoice(0), 3.0);
    s.apply(Action::PickChoice(2), 3.0);
    s.apply(Action::PickChoice(0), 3.0);
    s.apply(Action::PickChoice(0), 3.0);
    let pending = s.pending_ask().expect("still asking");
    assert!(pending.picked(0) && !pending.picked(1) && pending.picked(2));
    let sent = s.apply(Action::Reply(Reply::Allow), 3.0);
    assert_eq!(answer(&sent), Some((run, AskId("q".into()), Decision::Answer(vec![vec![1], vec![2, 0]]))));
}

#[test]
fn enter_takes_the_highlighted_option_and_arrows_stay_in_range() {
    let (mut s, run) = running();
    event(&mut s, run, questions());
    s.apply(Action::MoveChoice(-1), 3.0);
    assert_eq!(s.pending_ask().map(|a| a.cursor), Some(0));
    for _ in 0..5 {
        s.apply(Action::MoveChoice(1), 3.0);
    }
    assert_eq!(s.pending_ask().map(|a| a.cursor), Some(2));
    s.apply(Action::Reply(Reply::Allow), 3.0);
    assert_eq!(s.pending_ask().map(|a| (a.question, a.cursor)), Some((1, 0)));
    let skipped = s.apply(Action::Reply(Reply::Deny), 3.0);
    assert_eq!(answer(&skipped).map(|a| a.2), Some(Decision::Deny));
}

#[test]
fn asks_end_with_the_run_or_when_withdrawn() {
    let (mut s, run) = running();
    event(&mut s, run, permission("a", true));
    event(&mut s, run, permission("b", true));
    event(&mut s, run, EngineEvent::AskWithdrawn(AskId("a".into())));
    assert_eq!(s.pending_ask().map(|a| a.ask.id.clone()), Some(AskId("b".into())));
    event(&mut s, run, EngineEvent::Cancelled);
    assert!(s.pending_ask().is_none());
    event(&mut s, RunId::new(), permission("stray", true));
    assert!(s.pending_ask().is_none(), "asks of unknown runs are dropped");
}

#[test]
fn a_background_session_waits_on_the_timeline() {
    let (mut s, run) = running();
    s.apply(Action::NewConversation, 3.0);
    event(&mut s, run, permission("a", true));
    assert!(s.pending_ask().is_none(), "the session on screen asked nothing");
    let items: Vec<_> = s.timeline(3.0).into_iter().flat_map(|g| g.items).collect();
    assert!(items.iter().any(|i| i.waiting && !i.selected));
}

#[test]
fn the_permission_mode_travels_with_the_request_and_the_run() {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    assert_eq!(s.conversation().permission, PermissionMode::AcceptEdits);
    s.apply(Action::OpenPopover(PopoverKind::Permission), 0.0);
    assert!(commands(&s.apply(Action::SetPermissionMode(PermissionMode::Ask), 0.0)).is_empty());
    assert_eq!(s.popover, None);
    s.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
    let effects = s.apply(Action::Submit, 1.0);
    let run = match commands(&effects).as_slice() {
        [ClientCommand::SubmitPrompt { run, request }] => {
            assert_eq!(request.permission, PermissionMode::Ask);
            *run
        }
        other => panic!("unexpected {other:?}"),
    };
    let changed = s.apply(Action::SetPermissionMode(PermissionMode::Plan), 2.0);
    assert!(matches!(
        commands(&changed).as_slice(),
        [ClientCommand::SetPermissionMode { run: r, mode: PermissionMode::Plan }] if *r == run
    ));
}

#[test]
fn an_ask_rings_and_an_answer_clicks() {
    let (mut s, run) = running();
    let rang = event(&mut s, run, permission("a", true));
    assert!(rang.iter().any(|e| matches!(e, Effect::Sound(superfik_sound::Cue::Notification))));
    let answered = s.apply(Action::Reply(Reply::Allow), 3.0);
    assert!(answered.iter().any(|e| matches!(e, Effect::Sound(superfik_sound::Cue::Button))));
}

#[test]
fn back_returns_to_the_answered_question_with_its_pick_and_keeps_the_answers() {
    let (mut s, run) = running();
    event(&mut s, run, questions());
    s.apply(Action::Reply(Reply::Allow), 3.0);
    s.apply(Action::Reply(Reply::Back), 3.2);
    assert_eq!(s.pending_ask().map(|a| a.question), Some(0), "back to the first question");
    assert_eq!(s.pending_ask().map(|a| a.cursor), Some(0), "highlighting its pick");
    assert_eq!(s.pending_ask().map(|a| a.picks[0].clone()), Some(vec![0]), "its answer is kept");
    s.apply(Action::PickChoice(2), 3.3);
    assert_eq!(s.pending_ask().map(|a| a.question), Some(1), "the next question again");
    assert_eq!(s.pending_ask().map(|a| a.picks[0].clone()), Some(vec![2]));
}

#[test]
fn back_does_nothing_on_the_first_question_or_a_permission() {
    let (mut s, run) = running();
    event(&mut s, run, questions());
    s.apply(Action::Reply(Reply::Back), 3.0);
    assert_eq!(s.pending_ask().map(|a| a.question), Some(0));
    let (mut s, run) = running();
    event(&mut s, run, permission("p", true));
    s.apply(Action::Reply(Reply::Back), 3.0);
    assert!(s.pending_ask().is_some(), "a permission is not answered by Back");
}
