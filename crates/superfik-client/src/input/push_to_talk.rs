//! Holding left ⌘ to talk, like a radio: held alone for `PUSH_TO_TALK_HOLD`
//! it opens the microphone; letting go sends what was said. Any other key
//! or a click while it is held is a shortcut, not talk: it never starts,
//! and a talk already started is dropped. Pure; the shell feeds it the
//! key's presses and releases and the time of each frame.

use superfik_ui::theme::motion;

use crate::state::{Action, VoiceAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TalkKey {
    Talk,
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PushToTalk {
    held_since: Option<f64>,
    spoiled: bool,
    talking: bool,
}

impl PushToTalk {
    pub fn press(&mut self, key: TalkKey, now: f64) -> Option<Action> {
        match key {
            TalkKey::Talk => {
                if self.held_since.is_none() {
                    *self = Self { held_since: Some(now), ..Self::default() };
                }
                None
            }
            TalkKey::Other if self.held_since.is_some() => {
                self.spoiled = true;
                std::mem::take(&mut self.talking).then_some(Action::Voice(VoiceAction::DropTalk))
            }
            TalkKey::Other => None,
        }
    }

    pub fn release(&mut self, key: TalkKey) -> Option<Action> {
        if key != TalkKey::Talk {
            return None;
        }
        let talking = self.talking;
        *self = Self::default();
        talking.then_some(Action::Voice(VoiceAction::ReleaseTalk))
    }

    pub fn tick(&mut self, now: f64) -> Option<Action> {
        let since = self.held_since?;
        if self.spoiled || self.talking || now - since < motion::PUSH_TO_TALK_HOLD {
            return None;
        }
        self.talking = true;
        Some(Action::Voice(VoiceAction::HoldTalk))
    }

    pub fn blur(&mut self) -> Option<Action> {
        let talking = self.talking;
        *self = Self::default();
        talking.then_some(Action::Voice(VoiceAction::DropTalk))
    }

    pub fn is_waiting(&self) -> bool {
        self.held_since.is_some() && !self.spoiled && !self.talking
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(action: Option<Action>) -> Option<&'static str> {
        action.map(|a| match a {
            Action::Voice(VoiceAction::HoldTalk) => "hold",
            Action::Voice(VoiceAction::ReleaseTalk) => "release",
            Action::Voice(VoiceAction::DropTalk) => "drop",
            _ => "other",
        })
    }

    #[test]
    fn held_alone_it_talks_and_letting_go_sends() {
        let mut ptt = PushToTalk::default();
        assert_eq!(kind(ptt.press(TalkKey::Talk, 1.0)), None);
        assert_eq!(kind(ptt.tick(1.1)), None, "a tap is not talk");
        assert!(ptt.is_waiting());
        assert_eq!(kind(ptt.tick(1.0 + motion::PUSH_TO_TALK_HOLD)), Some("hold"));
        assert_eq!(kind(ptt.tick(3.0)), None, "once");
        assert_eq!(kind(ptt.release(TalkKey::Talk)), Some("release"));
        assert_eq!(kind(ptt.tick(9.0)), None);
    }

    #[test]
    fn a_shortcut_never_talks() {
        let mut ptt = PushToTalk::default();
        ptt.press(TalkKey::Talk, 1.0);
        assert_eq!(kind(ptt.press(TalkKey::Other, 1.05)), None);
        assert_eq!(kind(ptt.tick(5.0)), None);
        assert_eq!(kind(ptt.release(TalkKey::Talk)), None);
    }

    #[test]
    fn a_key_while_talking_drops_the_talk() {
        let mut ptt = PushToTalk::default();
        ptt.press(TalkKey::Talk, 1.0);
        ptt.tick(2.0);
        assert_eq!(kind(ptt.press(TalkKey::Other, 2.5)), Some("drop"));
        assert_eq!(kind(ptt.release(TalkKey::Talk)), None);
    }

    #[test]
    fn losing_the_window_drops_the_talk() {
        let mut ptt = PushToTalk::default();
        ptt.press(TalkKey::Talk, 1.0);
        ptt.tick(2.0);
        assert_eq!(kind(ptt.blur()), Some("drop"));
        assert_eq!(kind(ptt.blur()), None);
    }

    #[test]
    fn a_tap_and_a_quick_release_send_nothing() {
        let mut ptt = PushToTalk::default();
        ptt.press(TalkKey::Talk, 1.0);
        assert_eq!(kind(ptt.release(TalkKey::Talk)), None);
    }
}
