//! The voice agent's turn-taking: models first, then listening; a pause
//! sends what was heard; the reply is said sentence by sentence; listening
//! resumes once it is all said.

use std::path::PathBuf;

use superfik_core::{EngineEvent, RunId};
use superfik_protocol::{ClientCommand, ServerEvent};
use superfik_voice::{ModelKind, Voice, VoiceCall, VoiceEvent, VoicePrefs, VoiceProblem};

use super::tests::commands;
use super::*;

pub(super) fn session() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.preferences.language = Language::Portuguese;
    s
}

pub(super) fn device(s: &mut Session, event: VoiceEvent) -> Vec<Effect> {
    s.apply(Action::Voice(VoiceAction::Device(event)), 1.0)
}

pub(super) fn calls(effects: &[Effect]) -> Vec<VoiceCall> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Voice(call) => Some(call.clone()),
            _ => None,
        })
        .collect()
}

fn spoken(effects: &[Effect]) -> Vec<String> {
    calls(effects)
        .into_iter()
        .filter_map(|c| match c {
            VoiceCall::Speak { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

pub(super) fn listening() -> Session {
    let mut s = session();
    s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    device(&mut s, VoiceEvent::Ready);
    s
}

pub(super) fn say(s: &mut Session, text: &str) -> RunId {
    let effects = device(s, VoiceEvent::Heard { text: text.into(), ended: true });
    match commands(&effects).as_slice() {
        [ClientCommand::SubmitPrompt { run, .. }] => *run,
        other => panic!("unexpected effects {other:?}"),
    }
}

pub(super) fn reply(s: &mut Session, run: RunId, event: EngineEvent) -> Vec<Effect> {
    s.apply(Action::Server(ServerEvent::Run { run, event }), 2.0)
}

#[test]
fn turning_it_on_fetches_the_models_then_listens() {
    let mut s = session();
    let effects = s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    assert_eq!(calls(&effects), [VoiceCall::Prepare(VoicePrefs::default())]);
    assert_eq!(s.voice.phase, VoicePhase::Preparing(0.0));
    device(&mut s, VoiceEvent::Preparing { kind: ModelKind::Speech, fraction: 0.5 });
    let VoicePhase::Preparing(done) = s.voice.phase else { panic!("still preparing") };
    assert!((done - 0.83).abs() < 1e-4, "the recognizer is the larger download: {done}");
    let effects = device(&mut s, VoiceEvent::Ready);
    assert_eq!(calls(&effects), [VoiceCall::Listen(VoicePrefs::default())]);
    assert_eq!(s.voice.phase, VoicePhase::Listening);
}

#[test]
fn heard_words_fill_the_composer_after_the_draft_and_a_pause_sends_them() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("No README,".into())), 0.0);
    s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    device(&mut s, VoiceEvent::Ready);
    device(&mut s, VoiceEvent::Heard { text: "explique o".into(), ended: false });
    assert_eq!(s.conversation().composer.text(), "No README, explique o");
    let effects = device(&mut s, VoiceEvent::Heard { text: "explique o projeto".into(), ended: true });
    match commands(&effects).as_slice() {
        [ClientCommand::SubmitPrompt { request, .. }] => {
            let persona = superfik_core::spoken::Persona { name: "Piratinha", address: "Capitão" };
            assert_eq!(request.prompt, superfik_core::spoken::for_speech("No README, explique o projeto", persona));
        }
        other => panic!("unexpected effects {other:?}"),
    }
    assert!(calls(&effects).contains(&VoiceCall::Stop), "the microphone closes while the agent works");
    assert_eq!(s.voice.phase, VoicePhase::Waiting);
}

#[test]
fn a_pause_with_nothing_said_sends_nothing() {
    let mut s = listening();
    let effects = device(&mut s, VoiceEvent::Heard { text: String::new(), ended: true });
    assert!(commands(&effects).is_empty());
    assert_eq!(s.voice.phase, VoicePhase::Listening);
}

#[test]
fn the_reply_is_said_sentence_by_sentence_as_it_streams() {
    let mut s = listening();
    let run = say(&mut s, "Rode os testes");
    let first = reply(&mut s, run, EngineEvent::TextDelta("Rodei os testes do workspace. Dois".into()));
    assert_eq!(spoken(&first), ["Rodei os testes do workspace."]);
    assert!(
        calls(&first).iter().all(|c| !matches!(c, VoiceCall::Speak { voice, .. } if *voice != Voice::PmSanta)),
        "Santa speaks unless another voice was chosen"
    );
    assert_eq!(s.voice.phase, VoicePhase::Speaking);
    assert!(spoken(&reply(&mut s, run, EngineEvent::TextDelta(" falharam no parser".into()))).is_empty());
    let done = reply(&mut s, run, EngineEvent::Completed { duration_ms: 5, cost_usd: None });
    assert_eq!(spoken(&done), ["Dois falharam no parser."]);
    assert!(!calls(&done).iter().any(|c| matches!(c, VoiceCall::Listen(_))), "it listens once all is said");
    let said = device(&mut s, VoiceEvent::Spoken);
    assert_eq!(calls(&said), [VoiceCall::Listen(VoicePrefs::default())]);
    assert_eq!(s.voice.phase, VoicePhase::Listening);
}

#[test]
fn spoken_before_the_reply_ends_keeps_waiting() {
    let mut s = listening();
    let run = say(&mut s, "Oi");
    reply(&mut s, run, EngineEvent::TextDelta("Primeira frase já completa aqui. ".into()));
    assert!(calls(&device(&mut s, VoiceEvent::Spoken)).is_empty());
    assert_eq!(s.voice.phase, VoicePhase::Speaking);
}

#[test]
fn a_reply_with_nothing_to_say_listens_again_at_once() {
    let mut s = listening();
    let run = say(&mut s, "Oi");
    let done = reply(&mut s, run, EngineEvent::Completed { duration_ms: 5, cost_usd: None });
    assert_eq!(calls(&done), [VoiceCall::Listen(VoicePrefs::default())]);
}

#[test]
fn stopping_the_run_hushes_the_voice() {
    let mut s = listening();
    let run = say(&mut s, "Oi");
    reply(&mut s, run, EngineEvent::TextDelta("Uma frase bem longa para ser dita. ".into()));
    let effects = s.apply(Action::Cancel, 3.0);
    assert!(calls(&effects).contains(&VoiceCall::Hush));
    let ended = reply(&mut s, run, EngineEvent::Cancelled);
    assert!(spoken(&ended).is_empty(), "nothing more is said");
    assert_eq!(calls(&ended), [VoiceCall::Listen(VoicePrefs::default())]);
}

#[test]
fn other_conversations_are_not_read_aloud() {
    let mut s = listening();
    let run = say(&mut s, "Oi");
    s.voice = VoiceState::default();
    let effects = reply(&mut s, run, EngineEvent::TextDelta("Isto não deve ser dito em voz alta. ".into()));
    assert!(spoken(&effects).is_empty());
}

#[test]
fn turning_it_off_closes_the_microphone_and_hushes() {
    let mut s = listening();
    let effects = s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    assert_eq!(calls(&effects), [VoiceCall::Stop, VoiceCall::Hush]);
    assert!(!s.voice.is_on());
    assert!(device(&mut s, VoiceEvent::Heard { text: "tarde".into(), ended: true }).is_empty());
}

#[test]
fn a_failure_turns_it_off_with_a_notice() {
    let mut s = listening();
    device(&mut s, VoiceEvent::Failed(VoiceProblem::NoMicrophone("denied".into())));
    assert!(!s.voice.is_on());
    assert!(s.notice_text().is_some_and(|t| t.contains("microfone")));
}

#[test]
fn the_web_has_no_voice() {
    let mut s = Session { platform: Platform::WEB, ..session() };
    assert!(s.apply(Action::Voice(VoiceAction::Toggle), 0.0).iter().all(|e| !matches!(e, Effect::Voice(_))));
    assert!(!SettingsPane::offered(Platform::WEB).any(|p| p == SettingsPane::Voice));
}

#[test]
fn the_voice_command_toggles_it() {
    let mut s = session();
    s.apply(Action::RunCommand(Command::Voice), 0.0);
    assert!(s.voice.is_on());
}

#[test]
fn changing_a_voice_setting_while_on_prepares_again() {
    let mut s = listening();
    let chunk = superfik_voice::Chunk::Ms160;
    let effects = s.apply(Action::ChooseSetting(Setting::VoiceChunk(chunk)), 0.0);
    let prefs = s.preferences.voice_prefs();
    assert_eq!(prefs.recognizer, superfik_voice::Recognizer::Nemotron35(chunk));
    assert!(effects.iter().any(|e| matches!(e, Effect::Persist)));
    assert!(calls(&effects).contains(&VoiceCall::Prepare(prefs)));
}

#[test]
fn it_hears_the_interface_language() {
    let mut s =
        Session { preferences: Preferences { language: Language::English, ..Preferences::default() }, ..session() };
    let effects = s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    let english = VoicePrefs { language: superfik_voice::Language::English, ..VoicePrefs::default() };
    assert_eq!(calls(&effects), [VoiceCall::Prepare(english)]);
}

#[test]
fn changing_the_interface_language_while_on_prepares_again() {
    let mut s = listening();
    let effects = s.apply(Action::ChooseSetting(Setting::Language(Language::English)), 0.0);
    let english = VoicePrefs { language: superfik_voice::Language::English, ..VoicePrefs::default() };
    assert!(calls(&effects).contains(&VoiceCall::Prepare(english)), "{effects:?}");
}

#[test]
fn a_saved_english_checkpoint_listens_with_nemotron_3_5() {
    let chunk = superfik_voice::Chunk::Ms160;
    let saved = VoicePrefs { recognizer: superfik_voice::Recognizer::NemotronEnglish(chunk), ..VoicePrefs::default() };
    let mut s = session();
    s.preferences.voice = saved;
    let effects = s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    let expected = VoicePrefs { recognizer: superfik_voice::Recognizer::Nemotron35(chunk), ..VoicePrefs::default() };
    assert_eq!(calls(&effects), [VoiceCall::Prepare(expected)]);
}

#[test]
fn turned_off_in_settings_it_never_starts_and_stops_if_on() {
    let mut s = listening();
    let effects = s.apply(Action::ChooseSetting(Setting::VoiceEnabled(false)), 0.0);
    assert!(effects.iter().any(|e| matches!(e, Effect::Persist)));
    assert!(calls(&effects).contains(&VoiceCall::Stop), "{effects:?}");
    assert!(!s.voice.is_on());
    assert!(!s.offers_voice());
    assert!(calls(&s.apply(Action::Voice(VoiceAction::Toggle), 0.0)).is_empty());
    s.apply(Action::RunCommand(Command::Voice), 0.0);
    assert!(!s.voice.is_on());
}

#[test]
fn a_silent_pause_leaves_the_composer_alone() {
    let mut s = listening();
    s.conversation_mut().composer.set("typed while listening".to_owned());
    let effects = device(&mut s, VoiceEvent::Heard { text: String::new(), ended: true });
    assert!(effects.is_empty(), "{effects:?}");
    assert_eq!(s.conversation().composer.text(), "typed while listening");
}

#[test]
fn words_taken_back_before_the_pause_restore_the_draft() {
    let mut s = listening();
    device(&mut s, VoiceEvent::Heard { text: "hum".into(), ended: false });
    device(&mut s, VoiceEvent::Heard { text: String::new(), ended: true });
    assert_eq!(s.conversation().composer.text(), "");
}

#[test]
fn a_prompt_sent_by_voice_tells_the_agent_it_will_be_heard() {
    let mut s = listening();
    let effects = device(&mut s, VoiceEvent::Heard { text: "como está o build".into(), ended: true });
    let [ClientCommand::SubmitPrompt { request, .. }] = commands(&effects).as_slice() else { panic!("{effects:?}") };
    assert!(request.prompt.contains(superfik_core::spoken::MARK), "{}", request.prompt);
    assert!(request.prompt.ends_with("como está o build"), "{}", request.prompt);
    assert_eq!(s.conversation().turns.last().map(|t| t.user.text.as_str()), Some("como está o build"), "shown as said");
}

#[test]
fn a_typed_prompt_carries_no_voice_instructions() {
    let mut s = session();
    s.conversation_mut().composer.set("como está o build".to_owned());
    let effects = s.apply(Action::Submit, 0.0);
    let [ClientCommand::SubmitPrompt { request, .. }] = commands(&effects).as_slice() else { panic!("{effects:?}") };
    assert!(!request.prompt.contains(superfik_core::spoken::MARK));
}
