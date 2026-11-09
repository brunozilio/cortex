use std::path::PathBuf;

use superfik_core::{EngineEvent, RunId};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::*;
use crate::state::{Action, EditKey};

const REPLY: &str = "Here it is:\n```visual\n<svg viewBox=\"0 0 440 100\">\
<g class=\"node\" onclick=\"sendPrompt('Tell me about B')\"><rect x=\"0\" y=\"0\" width=\"10\" height=\"10\"/></g>\
</svg>\n```\nAnd more.";

fn session() -> Session {
    Session::new(Some(PathBuf::from("/repo")))
}

fn submit(s: &mut Session, text: &str) -> RunId {
    s.apply(Action::Edit(EditKey::Insert(text.into())), 1.0);
    match crate::state::tests::commands(&s.apply(Action::Submit, 1.0)).as_slice() {
        [ClientCommand::SubmitPrompt { run, .. }] => *run,
        other => panic!("unexpected {other:?}"),
    }
}

fn event(s: &mut Session, run: RunId, event: EngineEvent) {
    s.apply(Action::Server(ServerEvent::Run { run, event }), 2.0);
}

fn answered(reply: &str, done: bool) -> Session {
    let mut s = session();
    let run = submit(&mut s, "draw it");
    event(&mut s, run, EngineEvent::TextDelta(reply.into()));
    if done {
        event(&mut s, run, EngineEvent::Completed { duration_ms: 5, cost_usd: None });
    }
    s
}

const FIRST: VisualPrompt = VisualPrompt { turn: 0, visual: 0, prompt: 0 };

#[test]
fn a_click_sends_what_the_part_asks_and_keeps_the_draft() {
    let mut s = answered(REPLY, true);
    s.apply(Action::Edit(EditKey::Insert("half a thought".into())), 3.0);
    let effects = s.apply(Action::VisualPrompt(FIRST), 4.0);
    let sent = crate::state::tests::commands(&effects);
    assert!(matches!(sent.as_slice(), [ClientCommand::SubmitPrompt { .. }]), "{sent:?}");
    assert_eq!(s.conversation().turns[1].user.text, "Tell me about B");
    assert_eq!(s.conversation().composer.text(), "half a thought");
}

#[test]
fn nothing_is_sent_while_the_agent_still_works() {
    let mut s = answered(REPLY, false);
    let effects = s.apply(Action::VisualPrompt(FIRST), 3.0);
    assert!(crate::state::tests::commands(&effects).is_empty(), "{effects:?}");
    assert_eq!(s.conversation().turns.len(), 1);
}

#[test]
fn a_part_that_is_not_there_sends_nothing() {
    let mut s = answered(REPLY, true);
    for at in
        [VisualPrompt { prompt: 1, ..FIRST }, VisualPrompt { visual: 1, ..FIRST }, VisualPrompt { turn: 3, ..FIRST }]
    {
        assert!(crate::state::tests::commands(&s.apply(Action::VisualPrompt(at), 3.0)).is_empty(), "{at:?}");
    }
}
