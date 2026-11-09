//! Holding left ⌘ to talk: listening while held, pauses gather, letting go
//! sends, one message and then off.

use superfik_core::EngineEvent;
use superfik_protocol::ClientCommand;
use superfik_voice::{VoiceCall, VoiceEvent, VoicePrefs};

use super::tests::commands;
use super::tests_voice::{calls, device, listening, reply, say, session};
use super::*;

fn talk(s: &mut Session, action: VoiceAction) -> Vec<Effect> {
    s.apply(Action::Voice(action), 1.0)
}

fn holding() -> Session {
    let mut s = session();
    talk(&mut s, VoiceAction::HoldTalk);
    device(&mut s, VoiceEvent::Ready);
    s
}

#[test]
fn holding_talks_and_pauses_while_held_do_not_send() {
    let mut s = session();
    assert_eq!(calls(&talk(&mut s, VoiceAction::HoldTalk)), [VoiceCall::Prepare(VoicePrefs::default())]);
    assert_eq!(calls(&device(&mut s, VoiceEvent::Ready)), [VoiceCall::Listen(VoicePrefs::default())]);
    let paused = device(&mut s, VoiceEvent::Heard { text: "abre o".into(), ended: true });
    assert!(commands(&paused).is_empty(), "still held: {paused:?}");
    device(&mut s, VoiceEvent::Heard { text: "pull request".into(), ended: false });
    assert_eq!(s.conversation().composer.text(), "abre o pull request");
}

#[test]
fn letting_go_finishes_what_was_heard_and_sends_it() {
    let mut s = holding();
    device(&mut s, VoiceEvent::Heard { text: "abre o".into(), ended: false });
    assert_eq!(calls(&talk(&mut s, VoiceAction::ReleaseTalk)), [VoiceCall::Finish]);
    let sent = device(&mut s, VoiceEvent::Heard { text: "abre o pull request".into(), ended: true });
    assert!(matches!(commands(&sent).as_slice(), [ClientCommand::SubmitPrompt { .. }]), "{sent:?}");
    assert_eq!(s.voice.phase, VoicePhase::Waiting);
}

#[test]
fn letting_go_after_a_pause_sends_what_the_pause_kept() {
    let mut s = holding();
    device(&mut s, VoiceEvent::Heard { text: "roda os testes".into(), ended: true });
    talk(&mut s, VoiceAction::ReleaseTalk);
    let sent = device(&mut s, VoiceEvent::Heard { text: String::new(), ended: true });
    let [ClientCommand::SubmitPrompt { request, .. }] = commands(&sent).as_slice() else { panic!("{sent:?}") };
    assert!(request.prompt.ends_with("roda os testes"));
}

#[test]
fn letting_go_with_nothing_said_closes_the_mic() {
    let mut s = holding();
    talk(&mut s, VoiceAction::ReleaseTalk);
    let effects = device(&mut s, VoiceEvent::Heard { text: String::new(), ended: true });
    assert!(commands(&effects).is_empty());
    assert!(calls(&effects).contains(&VoiceCall::Stop));
    assert_eq!(s.voice.phase, VoicePhase::Off);
}

#[test]
fn after_the_reply_a_held_talk_does_not_keep_listening() {
    let mut s = holding();
    talk(&mut s, VoiceAction::ReleaseTalk);
    let run = say(&mut s, "que horas são");
    reply(&mut s, run, EngineEvent::TextDelta("São três horas.".into()));
    reply(&mut s, run, EngineEvent::Completed { duration_ms: 5, cost_usd: None });
    let said = device(&mut s, VoiceEvent::Spoken);
    assert!(!calls(&said).iter().any(|c| matches!(c, VoiceCall::Listen(_))), "{said:?}");
    assert_eq!(s.voice.phase, VoicePhase::Off);
}

#[test]
fn letting_go_before_the_models_are_ready_gives_up() {
    let mut s = session();
    talk(&mut s, VoiceAction::HoldTalk);
    let effects = talk(&mut s, VoiceAction::ReleaseTalk);
    assert!(calls(&effects).contains(&VoiceCall::Stop));
    assert_eq!(s.voice.phase, VoicePhase::Off);
}

#[test]
fn a_dropped_talk_sends_nothing() {
    let mut s = holding();
    device(&mut s, VoiceEvent::Heard { text: "abre".into(), ended: false });
    let effects = talk(&mut s, VoiceAction::DropTalk);
    assert!(commands(&effects).is_empty());
    assert_eq!(s.voice.phase, VoicePhase::Off);
}

#[test]
fn holding_while_a_conversation_is_on_changes_nothing() {
    let mut s = listening();
    assert!(talk(&mut s, VoiceAction::HoldTalk).is_empty());
    assert!(talk(&mut s, VoiceAction::ReleaseTalk).is_empty());
    assert_eq!(s.voice.phase, VoicePhase::Listening);
}
