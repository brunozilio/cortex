//! Maps the session onto the composer component and its click targets.

use superfik_core::{Activity, AskKind};
use superfik_i18n::Text;
use superfik_ui::Painter;
use superfik_ui::components::{Composer, ComposerHits, ComposerPart, ComposerProps, SendState};
use superfik_ui::emote::EmoteState;
use superfik_ui::theme::motion;

use super::target::{HitMap, Target};
use crate::state::{Conversation, Session};

pub struct ComposerView<'a> {
    pub session: &'a Session,
    pub origin: superfik_render::Point,
    pub hovered: Option<Target>,
    pub caret_visible: bool,
    pub scroll: f32,
    pub follow_caret: bool,
    pub now: f64,
}

pub fn emote_state(c: &Conversation, notice: bool, now: f64) -> EmoteState {
    if let Some(pending) = c.asks.front() {
        return match pending.ask.kind {
            AskKind::Permission { .. } => EmoteState::Alert,
            AskKind::Questions(_) => EmoteState::Question,
        };
    }
    if c.is_running() {
        return EmoteState::Thinking;
    }
    let last = c.turns.last().map(|t| &t.response);
    if notice || matches!(last.map(|r| &r.activity), Some(Activity::Failed(_))) {
        return EmoteState::Alert;
    }
    let just_finished = c.finished_at.is_some_and(|at| now - at < f64::from(motion::EMOTE_DONE_HOLD));
    if just_finished && matches!(last.map(|r| &r.activity), Some(Activity::Done)) {
        return EmoteState::Done;
    }
    EmoteState::Idle
}

impl ComposerView<'_> {
    pub fn paint(&self, p: &mut Painter, hits: &mut HitMap) -> ComposerHits {
        let s = self.session;
        let c = s.conversation();
        let previous_model = s
            .model_changed
            .as_ref()
            .map(|(label, at)| (label.as_str(), ((self.now - at) as f32 / motion::MORPH).clamp(0.0, 1.0)));
        let send = if s.is_running() {
            SendState::Running
        } else if c.composer.is_empty() {
            SendState::Disabled
        } else {
            SendState::Ready
        };
        let strings = s.strings();
        let placeholder = voice_placeholder(s).or_else(|| s.suggestion().map(str::to_owned)).unwrap_or_else(|| {
            strings.t(if c.turns.is_empty() { Text::ComposerEmpty } else { Text::ComposerFollowUp })
        });
        let permission_label = super::ask::mode_label(&strings, c.permission);
        let hovered = match self.hovered {
            Some(Target::Composer(part)) => Some(part),
            _ => None,
        };
        let highlights = highlights(s);
        let completion = s.completion();
        let props = ComposerProps {
            origin: self.origin,
            model: &s.model().label,
            previous_model,
            effort: strings.effort(c.effort),
            effort_level: c.effort.level(),
            ultra: c.effort.is_ultra(),
            edit: &c.composer,
            completion: completion.as_deref(),
            highlights: &highlights,
            placeholder: &placeholder,
            caret_visible: self.caret_visible,
            scroll: self.scroll,
            follow_caret: self.follow_caret,
            permission: (&permission_label, c.permission != superfik_core::PermissionMode::default()),
            permission_unsafe: c.permission == superfik_core::PermissionMode::Bypass,
            send,
            mic: mic_state(s),
            hovered,
            time: self.now as f32,
        };
        let c = Composer { props }.paint(p);
        hits.add(c.body, Target::Surface);
        hits.add(c.input, Target::Input);
        hits.input_scroll = Some((c.input_scroll, c.input_overflows));
        hits.input_text = c.input_text.clone();
        hits.effort_track = Some(c.effort_track);
        for (rect, part) in [
            (c.model, ComposerPart::Model),
            (c.effort, ComposerPart::Effort),
            (c.mic, ComposerPart::Mic),
            (c.image, ComposerPart::Image),
            (c.permission, ComposerPart::Permission),
            (c.send, ComposerPart::Send),
        ] {
            hits.add(rect, Target::Composer(part));
        }
        c
    }
}

pub fn mic_state(session: &Session) -> superfik_ui::components::MicState {
    use superfik_ui::components::MicState;
    if session.voice.is_on() {
        MicState::Live
    } else if session.voice.phase == crate::state::VoicePhase::Watching {
        MicState::Watching
    } else if session.offers_voice() {
        MicState::Idle
    } else {
        MicState::Hidden
    }
}

pub fn voice_placeholder(session: &Session) -> Option<String> {
    use crate::state::VoicePhase;
    let strings = session.strings();
    if !session.voice.is_on() {
        return None;
    }
    Some(match session.voice.phase {
        VoicePhase::Off | VoicePhase::Watching => return None,
        VoicePhase::Preparing(done) => strings.t(Text::VoicePreparing((done * 100.0).round() as usize)),
        VoicePhase::Listening => strings.t(Text::VoiceListening),
        VoicePhase::Waiting => strings.t(Text::VoiceWaiting),
        VoicePhase::Speaking => strings.t(Text::VoiceSpeaking),
    })
}

pub fn highlights(session: &crate::state::Session) -> Vec<superfik_ui::components::composer::Highlight> {
    use superfik_ui::components::composer::{Highlight, HighlightMark};
    let c = super::palette(session);
    session
        .composer_tokens()
        .into_iter()
        .filter_map(|token| {
            let (color, mark) = match token.pick {
                crate::state::Pick::Skill(skill) => {
                    let (icon, tint) = super::nodes::skills::icon(c, skill.icon);
                    let tint = tint.unwrap_or(c.skill);
                    (c.skill, HighlightMark::Icon(icon, tint))
                }
                crate::state::Pick::Connection(superfik_core::Connection { icon, .. })
                | crate::state::Pick::Plugin(superfik_core::PluginSummary { icon, .. }) => {
                    let brand = superfik_ui::brands::brand(icon);
                    (brand.color.mix(c.text, 0.25), HighlightMark::Brand(brand))
                }
                crate::state::Pick::Note(_) => (c.notes, HighlightMark::Icon(superfik_ui::icons::NOTE, c.notes)),
                crate::state::Pick::Command(_) => return None,
            };
            Some(Highlight { start: token.start, end: token.end, color, mark })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use superfik_core::{Turn, UserMessage};

    use super::*;
    use crate::state::{Action, EditKey};

    fn running() -> Session {
        let mut s = Session::new(Some(std::path::PathBuf::from("/r")));
        s.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
        s.apply(Action::Submit, 0.0);
        s
    }

    fn with_reply(s: &mut Session, activity: Activity, text: &str) {
        let c = s.conversation_mut();
        let mut turn = Turn::new(UserMessage { text: "go".into(), images: vec![], branch_caption: String::new() });
        turn.response.activity = activity;
        turn.response.text = text.into();
        c.turns = vec![turn];
    }

    #[test]
    fn thinks_while_running_and_idles_otherwise() {
        assert_eq!(emote_state(Session::default().conversation(), false, 0.0), EmoteState::Idle);
        let mut s = running();
        assert_eq!(emote_state(s.conversation(), false, 0.0), EmoteState::Thinking);
        with_reply(&mut s, Activity::Writing, "half");
        assert_eq!(emote_state(s.conversation(), false, 0.0), EmoteState::Thinking);
    }

    #[test]
    fn attention_alerts_once_idle() {
        let mut s = Session::new(Some(std::path::PathBuf::from("/r")));
        assert_eq!(emote_state(s.conversation(), true, 0.0), EmoteState::Alert);
        with_reply(&mut s, Activity::Failed("boom".into()), "");
        assert_eq!(emote_state(s.conversation(), false, 0.0), EmoteState::Alert);
    }

    #[test]
    fn a_finished_run_celebrates_briefly_even_when_the_reply_asks_something() {
        let mut s = Session::new(Some(std::path::PathBuf::from("/r")));
        with_reply(&mut s, Activity::Done, "What would you like to work on?");
        s.conversation_mut().finished_at = Some(10.0);
        assert_eq!(emote_state(s.conversation(), false, 11.0), EmoteState::Done);
        let later = 10.0 + f64::from(motion::EMOTE_DONE_HOLD) + 0.1;
        assert_eq!(emote_state(s.conversation(), false, later), EmoteState::Idle);
    }

    #[test]
    fn a_pending_ask_outranks_the_run() {
        use crate::state::PendingAsk;
        use superfik_core::{AgentAsk, AskId, Question};
        let mut s = running();
        let run = s.conversation().run.as_ref().expect("running").id;
        let ask = |kind| AgentAsk { id: AskId("a".into()), kind };
        let permission = ask(AskKind::Permission { tool: "Bash".into(), subject: "rm".into(), rememberable: false });
        s.conversation_mut().asks.push_back(PendingAsk::new(run, permission, 0.0));
        assert_eq!(emote_state(s.conversation(), false, 0.0), EmoteState::Alert);
        s.conversation_mut().asks.clear();
        let question = Question { header: "Branch".into(), text: "Which?".into(), options: vec![], multi: false };
        s.conversation_mut().asks.push_back(PendingAsk::new(run, ask(AskKind::Questions(vec![question])), 0.0));
        assert_eq!(emote_state(s.conversation(), false, 0.0), EmoteState::Question);
    }
}

#[cfg(test)]
mod mic_tests {
    use superfik_ui::components::MicState;

    use super::mic_state;
    use crate::state::{Action, Platform, Session, Setting, VoiceAction};

    #[test]
    fn the_mic_hides_rests_watches_and_lights() {
        let mut s = Session::default();
        assert_eq!(mic_state(&s), MicState::Idle);
        s.apply(Action::ChooseSetting(Setting::WakeEnabled(true)), 0.0);
        s.apply(Action::Voice(VoiceAction::Device(superfik_voice::VoiceEvent::Ready)), 0.0);
        assert_eq!(mic_state(&s), MicState::Watching);
        s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
        assert_eq!(mic_state(&s), MicState::Live);
        assert_eq!(mic_state(&Session { platform: Platform::WEB, ..Session::default() }), MicState::Hidden);
        s.apply(Action::ChooseSetting(Setting::VoiceEnabled(false)), 0.0);
        assert_eq!(mic_state(&s), MicState::Hidden);
    }
}
