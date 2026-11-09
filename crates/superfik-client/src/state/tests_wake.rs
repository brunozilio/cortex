//! The wake word: the microphone waits for it while no conversation is on,
//! saying it starts one, silence ends a conversation it started; and
//! writing the word in Settings › Voice.

use superfik_protocol::ClientCommand;
use superfik_voice::{VoiceCall, VoiceEvent, VoicePrefs, VoiceProblem, WakeWord};

use super::tests::commands;
use super::tests_voice::{calls, device, listening, session};
use super::*;

fn word() -> WakeWord {
    WakeWord::default()
}

fn watch_call() -> VoiceCall {
    VoiceCall::Watch { prefs: VoicePrefs::default(), word: word() }
}

fn watching() -> Session {
    let mut s = session();
    s.apply(Action::ChooseSetting(Setting::WakeEnabled(true)), 0.0);
    device(&mut s, VoiceEvent::Ready);
    s
}

fn woke(s: &mut Session, rest: &str, now: f64) -> Vec<Effect> {
    s.apply(Action::Voice(VoiceAction::Device(VoiceEvent::Woke { rest: rest.into() })), now)
}

#[test]
fn turning_the_wake_word_on_fetches_the_models_then_watches_without_lighting_the_mic() {
    let mut s = session();
    let effects = s.apply(Action::ChooseSetting(Setting::WakeEnabled(true)), 0.0);
    assert!(effects.iter().any(|e| matches!(e, Effect::Persist)));
    assert_eq!(calls(&effects), [VoiceCall::Prepare(VoicePrefs::default())]);
    assert!(!s.voice.is_on(), "the composer does not say it is preparing");
    assert_eq!(calls(&device(&mut s, VoiceEvent::Ready)), [watch_call()]);
    assert_eq!(s.voice.phase, VoicePhase::Watching);
    assert!(!s.voice.is_on());
}

#[test]
fn the_word_alone_starts_listening_with_a_chime() {
    let mut s = watching();
    let effects = woke(&mut s, "", 1.0);
    assert_eq!(calls(&effects), [VoiceCall::Listen(VoicePrefs::default())]);
    assert!(effects.iter().any(|e| matches!(e, Effect::Sound(superfik_sound::Cue::Notification))), "{effects:?}");
    assert_eq!(s.voice.phase, VoicePhase::Listening);
    assert!(s.voice.is_on());
}

#[test]
fn what_follows_the_word_goes_out_as_the_prompt() {
    let mut s = watching();
    let effects = woke(&mut s, "roda os testes", 1.0);
    assert!(matches!(commands(&effects).as_slice(), [ClientCommand::SubmitPrompt { .. }]), "{effects:?}");
    assert_eq!(s.voice.phase, VoicePhase::Waiting);
    assert!(!calls(&effects).iter().any(|c| matches!(c, VoiceCall::Listen(_))), "the agent works: the mic stays shut");
}

#[test]
fn the_wake_word_is_ignored_unless_watching() {
    let mut s = listening();
    assert!(woke(&mut s, "", 1.0).is_empty());
    let mut off = session();
    assert!(woke(&mut off, "oi", 1.0).is_empty());
    assert!(!off.voice.is_on());
}

#[test]
fn without_the_wake_word_the_mic_never_stays_open() {
    let mut s = listening();
    let effects = s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    assert_eq!(calls(&effects), [VoiceCall::Stop, VoiceCall::Hush]);
    assert_eq!(s.voice.phase, VoicePhase::Off);
}

#[test]
fn the_mic_button_still_talks_while_watching_and_turning_it_off_watches_again() {
    let mut s = watching();
    assert_eq!(calls(&s.apply(Action::Voice(VoiceAction::Toggle), 0.0)), [VoiceCall::Prepare(VoicePrefs::default())]);
    assert!(s.voice.is_on());
    assert_eq!(calls(&device(&mut s, VoiceEvent::Ready)), [VoiceCall::Listen(VoicePrefs::default())]);
    let off = s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    assert_eq!(calls(&off), [VoiceCall::Stop, VoiceCall::Hush, VoiceCall::Prepare(VoicePrefs::default())]);
    assert!(!s.voice.is_on());
    assert_eq!(calls(&device(&mut s, VoiceEvent::Ready)), [watch_call()]);
}

#[test]
fn silence_after_a_woken_conversation_returns_to_watching() {
    let mut s = watching();
    woke(&mut s, "", 1.0);
    let quiet = VoiceEvent::Heard { text: String::new(), ended: true };
    assert!(calls(&s.apply(Action::Voice(VoiceAction::Device(quiet.clone())), 10.0)).is_empty());
    s.apply(Action::Voice(VoiceAction::Device(VoiceEvent::Heard { text: "hum".into(), ended: false })), 12.0);
    assert!(calls(&s.apply(Action::Voice(VoiceAction::Device(quiet.clone())), 30.0)).is_empty(), "heard at 12 s");
    let back = s.apply(Action::Voice(VoiceAction::Device(quiet)), 33.0);
    assert_eq!(calls(&back), [VoiceCall::Stop, VoiceCall::Hush, VoiceCall::Prepare(VoicePrefs::default())]);
    assert!(!s.voice.is_on());
}

#[test]
fn silence_never_ends_a_conversation_the_mic_button_started() {
    let mut s = listening();
    s.preferences.wake.enabled = true;
    let quiet = VoiceEvent::Heard { text: String::new(), ended: true };
    assert!(calls(&s.apply(Action::Voice(VoiceAction::Device(quiet)), 100.0)).is_empty());
    assert_eq!(s.voice.phase, VoicePhase::Listening);
}

#[test]
fn turning_the_wake_word_or_the_voice_off_closes_the_mic() {
    for setting in [Setting::WakeEnabled(false), Setting::VoiceEnabled(false)] {
        let mut s = watching();
        let effects = s.apply(Action::ChooseSetting(setting), 0.0);
        assert!(calls(&effects).contains(&VoiceCall::Stop), "{setting:?}: {effects:?}");
        assert!(!calls(&effects).iter().any(|c| matches!(c, VoiceCall::Prepare(_))), "{setting:?}");
        assert_eq!(s.voice.phase, VoicePhase::Off);
    }
}

#[test]
fn turning_the_voice_back_on_watches_again() {
    let mut s = watching();
    s.apply(Action::ChooseSetting(Setting::VoiceEnabled(false)), 0.0);
    let effects = s.apply(Action::ChooseSetting(Setting::VoiceEnabled(true)), 0.0);
    assert_eq!(calls(&effects), [VoiceCall::Prepare(VoicePrefs::default())]);
}

#[test]
fn a_new_word_is_watched_for_at_once() {
    let mut s = watching();
    let jarvis = WakeWord::new("Jarvis").expect("word");
    let effects = s.apply(Action::ChooseSetting(Setting::WakeWord(jarvis)), 0.0);
    assert_eq!(calls(&effects), [VoiceCall::Watch { prefs: VoicePrefs::default(), word: jarvis }]);
}

#[test]
fn restored_preferences_with_the_wake_word_start_watching() {
    let mut s = session();
    let prefs = Preferences {
        language: Language::Portuguese,
        wake: superfik_voice::WakePrefs { enabled: true, word: word() },
        ..Preferences::default()
    };
    assert_eq!(calls(&s.apply(Action::RestorePreferences(prefs), 0.0)), [VoiceCall::Prepare(VoicePrefs::default())]);
    assert!(calls(&s.apply(Action::RestorePreferences(prefs), 0.0)).is_empty(), "already watching");
}

#[test]
fn a_failure_while_watching_stops_it_until_asked_again() {
    let mut s = watching();
    let effects = device(&mut s, VoiceEvent::Failed(VoiceProblem::NoMicrophone("none".into())));
    assert!(!calls(&effects).iter().any(|c| matches!(c, VoiceCall::Prepare(_))), "{effects:?}");
    assert_eq!(s.voice.phase, VoicePhase::Off);
    assert!(s.notice.is_some());
}

#[test]
fn the_web_never_watches() {
    let mut s = Session { platform: Platform::WEB, ..session() };
    assert!(calls(&s.apply(Action::ChooseSetting(Setting::WakeEnabled(true)), 0.0)).is_empty());
}

#[test]
fn the_shell_launch_with_the_wake_word_saved_watches() {
    let mut s = Session { platform: Platform::DESKTOP, ..Session::launching() };
    let prefs =
        Preferences { wake: superfik_voice::WakePrefs { enabled: true, word: word() }, ..Preferences::default() };
    let effects = s.apply(Action::RestorePreferences(prefs), 0.0);
    assert!(calls(&effects).iter().any(|c| matches!(c, VoiceCall::Prepare(_))), "{effects:?}");
    let ready = device(&mut s, VoiceEvent::Ready);
    assert!(calls(&ready).iter().any(|c| matches!(c, VoiceCall::Watch { .. })), "{ready:?}");
}
