//! Writing in Settings › Voice: the wake word and what the agent calls the
//! user, and the spoken prompt calling the user that.

use superfik_protocol::ClientCommand;
use superfik_voice::{Address, VoiceEvent, WakeWord};

use super::tests::commands;
use super::tests_voice::{device, session};
use super::*;

fn field_action(s: &mut Session, action: VoiceFieldAction) -> Vec<Effect> {
    s.apply(Action::VoiceField(action), 0.0)
}

fn retype(s: &mut Session, text: &str) {
    for _ in 0..WakeWord::DEFAULT.len().max(Address::DEFAULT.len()) {
        field_action(s, VoiceFieldAction::Edit(EditKey::Backspace));
    }
    field_action(s, VoiceFieldAction::Edit(EditKey::Insert(text.into())));
}

fn written(s: &Session) -> Option<(VoiceField, String)> {
    s.voice_field_edit.as_ref().map(|(field, edit)| (*field, edit.text().to_owned()))
}

#[test]
fn the_word_is_written_in_place_and_enter_keeps_it() {
    let mut s = session();
    field_action(&mut s, VoiceFieldAction::Focus(VoiceField::WakeWord));
    assert_eq!(written(&s), Some((VoiceField::WakeWord, "Piratinha".to_owned())));
    retype(&mut s, " Jarvis ");
    let effects = field_action(&mut s, VoiceFieldAction::Save);
    assert!(effects.iter().any(|e| matches!(e, Effect::Persist)), "{effects:?}");
    assert_eq!(s.preferences.wake.word.as_str(), "Jarvis");
    assert!(s.voice_field_edit.is_none());
}

#[test]
fn escape_leaves_the_word_as_it_was() {
    let mut s = session();
    field_action(&mut s, VoiceFieldAction::Focus(VoiceField::WakeWord));
    retype(&mut s, "Jarvis");
    assert!(field_action(&mut s, VoiceFieldAction::Cancel).is_empty());
    assert_eq!(s.preferences.wake.word, WakeWord::default());
    assert!(s.voice_field_edit.is_none());
}

#[test]
fn an_emptied_word_goes_back_to_piratinha() {
    let mut s = session();
    s.preferences.wake.word = WakeWord::new("Jarvis").expect("word");
    field_action(&mut s, VoiceFieldAction::Focus(VoiceField::WakeWord));
    retype(&mut s, "   ");
    field_action(&mut s, VoiceFieldAction::Save);
    assert_eq!(s.preferences.wake.word, WakeWord::default());
}

#[test]
fn closing_settings_keeps_what_was_written() {
    let mut s = session();
    s.apply(Action::OpenSettings, 0.0);
    field_action(&mut s, VoiceFieldAction::Focus(VoiceField::WakeWord));
    retype(&mut s, "Jarvis");
    s.apply(Action::CloseSettings, 0.0);
    assert_eq!(s.preferences.wake.word.as_str(), "Jarvis");
    assert!(s.voice_field_edit.is_none());
}

#[test]
fn the_user_is_captain_until_renamed_and_an_emptied_name_goes_back() {
    let mut s = session();
    assert_eq!(s.preferences.voice_address.as_str(), "Capitão");
    field_action(&mut s, VoiceFieldAction::Focus(VoiceField::Address));
    assert_eq!(written(&s), Some((VoiceField::Address, "Capitão".to_owned())));
    retype(&mut s, "Chefe");
    let effects = field_action(&mut s, VoiceFieldAction::Save);
    assert!(effects.iter().any(|e| matches!(e, Effect::Persist)), "{effects:?}");
    assert_eq!(s.preferences.voice_address.as_str(), "Chefe");
    field_action(&mut s, VoiceFieldAction::Focus(VoiceField::Address));
    retype(&mut s, "  ");
    field_action(&mut s, VoiceFieldAction::Save);
    assert_eq!(s.preferences.voice_address, Address::default());
}

#[test]
fn moving_to_the_other_field_keeps_the_first() {
    let mut s = session();
    field_action(&mut s, VoiceFieldAction::Focus(VoiceField::WakeWord));
    retype(&mut s, "Jarvis");
    let effects = field_action(&mut s, VoiceFieldAction::Focus(VoiceField::Address));
    assert!(effects.iter().any(|e| matches!(e, Effect::Persist)), "{effects:?}");
    assert_eq!(s.preferences.wake.word.as_str(), "Jarvis");
    assert_eq!(written(&s), Some((VoiceField::Address, "Capitão".to_owned())));
}

#[test]
fn a_spoken_prompt_names_the_agent_and_calls_the_user_as_chosen() {
    let mut s = session();
    s.preferences.voice_address = Address::new("Chefe").expect("address");
    s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    device(&mut s, VoiceEvent::Ready);
    let effects = device(&mut s, VoiceEvent::Heard { text: "roda os testes".into(), ended: true });
    let [ClientCommand::SubmitPrompt { request, .. }] = commands(&effects).as_slice() else {
        panic!("unexpected effects {effects:?}")
    };
    assert!(request.prompt.contains("you are called \"Piratinha\""), "{}", request.prompt);
    assert!(request.prompt.contains("call them \"Chefe\""), "{}", request.prompt);
    assert!(request.prompt.ends_with("roda os testes"));
}
