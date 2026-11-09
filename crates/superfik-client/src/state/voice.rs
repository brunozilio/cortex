//! The voice agent: the user speaks, the words fill the composer as they
//! are heard and go out as a prompt when the user pauses; the reply is said
//! sentence by sentence as it streams in; once it is all said, the agent
//! listens again. Half duplex: the microphone is closed while the agent
//! thinks and speaks, so it never hears itself.
//!
//! The rules are here; the device (`superfik_voice::VoiceDevice`) only
//! listens, speaks and reports. `Effect::Voice` carries the calls,
//! `Action::Voice(VoiceAction::Device(_))` the reports.

use superfik_core::ConversationId;
use superfik_voice::{ModelKind, Sentences, VoiceCall, VoiceEvent, VoiceProblem};

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum VoicePhase {
    #[default]
    Off,
    Preparing(f32),
    Watching,
    Listening,
    Waiting,
    Speaking,
}

#[derive(Debug, Clone)]
pub enum VoiceAction {
    Toggle,
    Device(VoiceEvent),
    HoldTalk,
    ReleaseTalk,
    DropTalk,
}

#[derive(Debug, Clone, Default)]
pub struct VoiceState {
    pub phase: VoicePhase,
    pub(super) conversation: Option<ConversationId>,
    pub(super) draft: String,
    pub(super) reply: Sentences,
    pub(super) replied: bool,
    pub(super) speaking: bool,
    pub(super) goal: Goal,
    pub(super) woken: bool,
    pub(super) heard_at: Option<f64>,
    pub(super) partial: bool,
    pub(super) push: Push,
    pub(super) said: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Push {
    #[default]
    No,
    Held,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Goal {
    #[default]
    Talk,
    Watch,
}

impl VoiceState {
    pub fn is_on(&self) -> bool {
        match self.phase {
            VoicePhase::Off | VoicePhase::Watching => false,
            VoicePhase::Preparing(_) => self.goal == Goal::Talk,
            VoicePhase::Listening | VoicePhase::Waiting | VoicePhase::Speaking => true,
        }
    }

    pub fn is_watching(&self) -> bool {
        match self.phase {
            VoicePhase::Watching => true,
            VoicePhase::Preparing(_) => self.goal == Goal::Watch,
            _ => false,
        }
    }
}

const RECOGNIZER_SHARE: f32 = 0.66;

impl Session {
    pub(super) fn voice(&mut self, action: VoiceAction, now: f64) -> Vec<Effect> {
        match action {
            VoiceAction::Toggle => self.toggle_voice(),
            VoiceAction::Device(event) => self.voice_device(event, now),
            VoiceAction::HoldTalk => self.hold_talk(),
            VoiceAction::ReleaseTalk => self.release_talk(),
            VoiceAction::DropTalk => self.drop_talk(),
        }
    }

    pub fn offers_voice(&self) -> bool {
        self.platform.voice && self.preferences.voice_enabled
    }

    pub(super) fn toggle_voice(&mut self) -> Vec<Effect> {
        if self.voice.is_on() {
            return self.stop_voice();
        }
        if !self.offers_voice() {
            return Vec::new();
        }
        self.voice = VoiceState { phase: VoicePhase::Preparing(0.0), ..VoiceState::default() };
        vec![Effect::Voice(VoiceCall::Prepare(self.preferences.voice_prefs()))]
    }

    fn voice_device(&mut self, event: VoiceEvent, now: f64) -> Vec<Effect> {
        match event {
            VoiceEvent::Preparing { kind, fraction } if matches!(self.voice.phase, VoicePhase::Preparing(_)) => {
                let done = match kind {
                    ModelKind::Recognizer => fraction * RECOGNIZER_SHARE,
                    ModelKind::Speech => RECOGNIZER_SHARE + fraction * (1.0 - RECOGNIZER_SHARE),
                };
                self.voice.phase = VoicePhase::Preparing(done.clamp(0.0, 1.0));
                Vec::new()
            }
            VoiceEvent::Ready if matches!(self.voice.phase, VoicePhase::Preparing(_)) => match self.voice.goal {
                Goal::Talk => self.listen(),
                Goal::Watch => self.watch(),
            },
            VoiceEvent::Heard { text, ended } if self.voice.phase == VoicePhase::Listening => {
                if self.voice.push != Push::No {
                    return self.push_heard(&text, ended, now);
                }
                if self.fell_quiet(&text, now) {
                    return self.stop_voice();
                }
                let silent = text.trim().is_empty();
                if silent && ended && !self.voice.partial {
                    return Vec::new();
                }
                self.voice.partial = !silent && !ended;
                self.heard(&text, ended, now)
            }
            VoiceEvent::Woke { rest } => self.woke(&rest, now),
            VoiceEvent::Spoken if self.voice.is_on() => {
                self.voice.speaking = false;
                if self.voice.replied { self.after_reply() } else { Vec::new() }
            }
            VoiceEvent::Failed(problem) => {
                self.notice = Some(super::Notice::Voice(problem));
                self.voice = VoiceState::default();
                vec![Effect::Voice(VoiceCall::Stop), Effect::Voice(VoiceCall::Hush)]
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn listen(&mut self) -> Vec<Effect> {
        let draft = self.conversation().composer.text().trim_end().to_owned();
        let (woken, push) = (self.voice.woken, self.voice.push);
        self.voice = VoiceState { phase: VoicePhase::Listening, draft, woken, push, ..VoiceState::default() };
        vec![Effect::Voice(VoiceCall::Listen(self.preferences.voice_prefs()))]
    }

    pub(super) fn after_reply(&mut self) -> Vec<Effect> {
        if self.voice.push == Push::No { self.listen() } else { self.stop_voice() }
    }

    pub(super) fn stop_voice(&mut self) -> Vec<Effect> {
        self.voice = VoiceState::default();
        let mut effects = vec![Effect::Voice(VoiceCall::Stop), Effect::Voice(VoiceCall::Hush)];
        effects.extend(self.start_watch());
        effects
    }

    pub(super) fn heard(&mut self, text: &str, ended: bool, now: f64) -> Vec<Effect> {
        self.show_heard(text);
        if !ended || text.trim().is_empty() {
            return Vec::new();
        }
        self.send_heard(now)
    }

    pub(super) fn show_heard(&mut self, text: &str) {
        let draft = &self.voice.draft;
        let joined = match (draft.is_empty(), text.is_empty()) {
            (true, _) => text.to_owned(),
            (false, true) => draft.clone(),
            (false, false) => format!("{draft} {text}"),
        };
        self.conversation_mut().composer.set(joined);
    }

    pub(super) fn send_heard(&mut self, now: f64) -> Vec<Effect> {
        let mut effects = self.submit(now);
        if self.conversation().is_running() {
            self.voice.phase = VoicePhase::Waiting;
            self.voice.conversation = Some(self.active);
            self.voice.draft.clear();
            effects.push(Effect::Voice(VoiceCall::Stop));
        } else {
            self.voice.draft = self.conversation().composer.text().trim_end().to_owned();
        }
        effects
    }

    pub(super) fn voice_settings_changed(&mut self) -> Vec<Effect> {
        if !self.voice.is_on() {
            return Vec::new();
        }
        let woken = self.voice.woken;
        self.voice = VoiceState { phase: VoicePhase::Preparing(0.0), woken, ..VoiceState::default() };
        vec![
            Effect::Voice(VoiceCall::Stop),
            Effect::Voice(VoiceCall::Hush),
            Effect::Voice(VoiceCall::Prepare(self.preferences.voice_prefs())),
        ]
    }
}

pub(super) fn problem_text(problem: &VoiceProblem, strings: &superfik_i18n::Strings) -> String {
    use superfik_i18n::Text;
    match problem {
        VoiceProblem::NoMicrophone(_) => strings.t(Text::VoiceNoMicrophone),
        VoiceProblem::Download(why) | VoiceProblem::Model(why) => strings.t(Text::VoiceFailed(why)),
    }
}
