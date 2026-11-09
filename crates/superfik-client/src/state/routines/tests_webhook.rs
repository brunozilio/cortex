//! A saved routine's webhook in the client: switched with the form, its
//! call copied without the session keeping the token.

use std::path::PathBuf;

use superfik_core::routine::{RoutineBook, RoutineEdit, RoutineId, WebhookId, WebhookToken};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::super::tests::commands;
use super::super::{Action, Effect, Session};
use super::*;

fn session(webhook: bool) -> (Session, Option<WebhookId>) {
    let mut book = RoutineBook::default();
    let draft = superfik_core::routine::RoutineDraft {
        name: "Orders".into(),
        brief: "Thank the buyer".into(),
        schedule: superfik_core::routine::Schedule::Every { minutes: 60 },
        autonomy: superfik_core::routine::Autonomy::Draft,
        model: None,
        connections: Vec::new(),
        answers: false,
    };
    book.apply(&[RoutineEdit::Create { draft }, RoutineEdit::Webhook { id: RoutineId(0), on: webhook }], 0);
    let hook = book.routines[0].webhook;
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.apply(Action::Server(ServerEvent::Routines { path: "/repo".into(), book }), 0.0);
    (s, hook)
}

fn routine(s: &mut Session, action: RoutineAction) -> Vec<Effect> {
    s.apply(Action::Routines(action), 0.0)
}

fn sent_edits(effects: &[Effect]) -> Vec<RoutineEdit> {
    commands(effects)
        .into_iter()
        .flat_map(|c| match c {
            ClientCommand::EditRoutines { edits, .. } => edits.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn only_a_saved_routine_switches_its_webhook_and_only_on_save() {
    let (mut s, _) = session(false);
    routine(&mut s, RoutineAction::New);
    routine(&mut s, RoutineAction::ToggleWebhook);
    assert!(!s.routines.form.as_ref().is_some_and(|f| f.webhook), "a new routine has no address yet");

    routine(&mut s, RoutineAction::Revise(RoutineId(0)));
    routine(&mut s, RoutineAction::ToggleWebhook);
    let edits = sent_edits(&routine(&mut s, RoutineAction::Save));
    assert!(matches!(edits.as_slice(), [RoutineEdit::Revise { .. }, RoutineEdit::Webhook { on: true, .. }]));
}

#[test]
fn saving_without_touching_the_webhook_leaves_it_alone() {
    let (mut s, _) = session(true);
    routine(&mut s, RoutineAction::Revise(RoutineId(0)));
    let edits = sent_edits(&routine(&mut s, RoutineAction::Save));
    assert!(matches!(edits.as_slice(), [RoutineEdit::Revise { .. }]));
}

#[test]
fn copying_asks_for_the_token_and_copies_the_call_without_keeping_it() {
    let (mut s, hook) = session(true);
    let hook = hook.unwrap_or_else(|| panic!("no webhook"));
    let effects = routine(&mut s, RoutineAction::CopyWebhook(RoutineId(0)));
    assert!(
        commands(&effects)
            .iter()
            .any(|c| matches!(c, ClientCommand::RevealWebhook { routine, .. } if *routine == RoutineId(0)))
    );

    let token = WebhookToken("tok_123".into());
    let arrived = ServerEvent::RoutineWebhook { path: "/repo".into(), routine: RoutineId(0), hook, token };
    let effects = s.apply(Action::Server(arrived.clone()), 1.0);
    let copied: Vec<&String> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::CopyText(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(copied.len(), 1);
    assert!(copied[0].contains(&format!("/hooks/{hook}")) && copied[0].contains("Bearer tok_123"));
    assert_eq!(s.routines.copied, Some((RoutineId(0), 1.0)));
    assert!(!format!("{:?}", s.routines).contains("tok_123"), "the session keeps no token");

    let again = s.apply(Action::Server(arrived), 2.0);
    assert!(!again.iter().any(|e| matches!(e, Effect::CopyText(_))));
}

#[test]
fn a_routine_without_a_webhook_has_nothing_to_copy() {
    let (mut s, _) = session(false);
    assert!(routine(&mut s, RoutineAction::CopyWebhook(RoutineId(0))).is_empty());
}
