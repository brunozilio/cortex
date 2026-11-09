//! Maps the ask on screen onto the ask card, in the composer's place.

use superfik_core::{AskKind, PermissionMode};
use superfik_i18n::{Strings, Text};
use superfik_ui::Painter;
use superfik_ui::components::{AskButton, AskCard, AskCardProps, AskOption, AskPart};
use superfik_ui::emote::EmoteState;
use superfik_ui::theme::motion;

use super::target::{AskTarget, HitMap, Target};
use crate::state::{PendingAsk, Primary, Reply, Session};

const KEY_ALLOW: &str = "↵";
const KEY_DENY: &str = "esc";
const KEY_BACK: &str = "←";

pub struct AskView<'a> {
    pub session: &'a Session,
    pub ask: &'a PendingAsk,
    pub composer: superfik_render::Rect,
    pub hovered: Option<Target>,
    pub emote_from: Option<(EmoteState, f32)>,
    pub now: f64,
}

struct Texts {
    title: String,
    counter: Option<String>,
    subject: Option<String>,
    question: Option<String>,
    hint: Option<String>,
    buttons: Vec<(String, &'static str, bool, Reply)>,
}

impl AskView<'_> {
    pub fn extra_height(&self, p: &mut Painter) -> f32 {
        let texts = self.texts();
        let options = self.options();
        let buttons = buttons(&texts);
        let card = AskCard { props: self.props(&texts, &options, &buttons) };
        card.height(p) - self.composer.h
    }

    pub fn paint(&self, p: &mut Painter, hits: &mut HitMap) {
        let texts = self.texts();
        let options = self.options();
        let buttons = buttons(&texts);
        let painted = AskCard { props: self.props(&texts, &options, &buttons) }.paint(p);
        hits.add(painted.body, Target::Surface);
        for (i, rect) in painted.options.into_iter().enumerate() {
            hits.add(rect, Target::Ask(AskTarget::Option(i)));
        }
        for (rect, (.., reply)) in painted.buttons.into_iter().zip(&texts.buttons) {
            hits.add(rect, Target::Ask(AskTarget::Reply(*reply)));
        }
    }

    fn props<'b>(
        &self,
        texts: &'b Texts,
        options: &'b [AskOption<'b>],
        buttons: &'b [AskButton<'b>],
    ) -> AskCardProps<'b> {
        let hovered = match self.hovered {
            Some(Target::Ask(AskTarget::Option(i))) => Some(AskPart::Option(i)),
            Some(Target::Ask(AskTarget::Reply(reply))) => {
                texts.buttons.iter().position(|(.., r)| *r == reply).map(AskPart::Button)
            }
            _ => None,
        };
        let has_options = !options.is_empty();
        AskCardProps {
            x: self.composer.x,
            bottom: self.composer.bottom(),
            emote: EmoteState::Question,
            emote_from: self.emote_from,
            title: &texts.title,
            counter: texts.counter.as_deref(),
            subject: texts.subject.as_deref(),
            question: texts.question.as_deref(),
            options,
            cursor: has_options.then_some(self.ask.cursor),
            hint: texts.hint.as_deref(),
            buttons,
            hovered,
            progress: ((self.now - self.ask.arrived_at) as f32 / motion::CONTENT_SWAP).clamp(0.0, 1.0),
            time: self.now as f32,
        }
    }

    fn texts(&self) -> Texts {
        let s = self.session.strings();
        let waiting = self.session.conversation().asks.len();
        match &self.ask.ask.kind {
            AskKind::Permission { tool, subject, .. } => Texts {
                title: s.t(Text::AskPermission(tool)),
                counter: (waiting > 1).then(|| s.t(Text::AskOf(1, waiting))),
                subject: (!subject.is_empty()).then(|| subject.clone()),
                question: None,
                hint: self.ask.delegated.then(|| s.t(Text::WaitingForApprover)),
                buttons: self
                    .session
                    .may_delegate(self.ask)
                    .then_some(Reply::Delegate)
                    .into_iter()
                    .chain(self.ask.replies())
                    .map(|reply| reply_button(&s, reply, false, self.session.platform.primary))
                    .collect(),
            },
            AskKind::Questions(questions) => {
                let question = &questions[self.ask.question.min(questions.len() - 1)];
                let last = self.ask.question + 1 >= questions.len();
                Texts {
                    title: question.header.clone(),
                    counter: (questions.len() > 1).then(|| s.t(Text::AskOf(self.ask.question + 1, questions.len()))),
                    subject: None,
                    question: Some(question.text.clone()),
                    hint: question.multi.then(|| s.t(Text::AskPickMany)),
                    buttons: {
                        let mut buttons = vec![
                            reply_button(&s, Reply::Deny, true, self.session.platform.primary),
                            (s.t(if last { Text::AskAnswer } else { Text::AskNext }), KEY_ALLOW, true, Reply::Allow),
                        ];
                        if self.ask.question > 0 {
                            buttons.insert(0, reply_button(&s, Reply::Back, false, self.session.platform.primary));
                        }
                        buttons
                    },
                }
            }
        }
    }

    fn options(&self) -> Vec<AskOption<'_>> {
        let Some(question) = self.ask.current_question() else { return Vec::new() };
        question
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| AskOption { label: &o.label, description: &o.description, picked: self.ask.picked(i) })
            .collect()
    }
}

fn buttons(texts: &Texts) -> Vec<AskButton<'_>> {
    texts.buttons.iter().map(|(label, key, primary, _)| AskButton { label, key, primary: *primary }).collect()
}

fn reply_button(s: &Strings, reply: Reply, question: bool, primary: Primary) -> (String, &'static str, bool, Reply) {
    match reply {
        Reply::Allow => (s.t(Text::AskAllow), KEY_ALLOW, true, reply),
        Reply::AllowAlways => (s.t(Text::AskAlways), always_key(primary), false, reply),
        Reply::Deny => (s.t(if question { Text::AskSkip } else { Text::AskDeny }), KEY_DENY, false, reply),
        Reply::Back => (s.t(Text::AskBack), KEY_BACK, false, reply),
        Reply::Delegate => (s.t(Text::AskApprover), "", false, reply),
    }
}

fn always_key(primary: Primary) -> &'static str {
    match primary {
        Primary::Command => "⌘↵",
        Primary::Control => "Ctrl+↵",
    }
}

#[cfg(test)]
#[test]
fn allow_always_names_the_platforms_command_key() {
    use crate::state::Platform;
    assert_eq!(always_key(Platform::DESKTOP.primary), "⌘↵");
    assert_eq!(always_key(Platform::WINDOWS.primary), "Ctrl+↵");
    assert!(always_key(Primary::Control).starts_with(Primary::Control.label()));
}

pub fn mode_label(s: &Strings, mode: PermissionMode) -> String {
    s.t(match mode {
        PermissionMode::Ask => Text::PermissionAsk,
        PermissionMode::AcceptEdits => Text::PermissionEdits,
        PermissionMode::Plan => Text::PermissionPlan,
        PermissionMode::Bypass => Text::PermissionBypass,
    })
}
