//! The kinds of channel Superfik speaks, and what each protocol allows:
//! its send ceilings, whether it receives through a public webhook, and
//! the window in which a business may answer a person.

use serde::{Deserialize, Serialize};

use super::limits::Limits;

const HOUR_MS: i64 = 60 * 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    WhatsappWeb,
    Email,
    WhatsappCloud,
    Instagram,
    X,
    Linkedin,
}

impl ChannelKind {
    pub fn limits(self) -> Limits {
        match self {
            Self::WhatsappWeb | Self::Email => Limits::default(),
            Self::WhatsappCloud => Limits { per_hour: 1_000, per_day: 10_000, new_per_day: 250 },
            Self::Instagram => Limits { per_hour: 200, per_day: 1_000, new_per_day: 50 },
            Self::X => Limits { per_hour: 15, per_day: 100, new_per_day: 20 },
            Self::Linkedin => Limits { per_hour: 20, per_day: 100, new_per_day: 25 },
        }
    }

    pub fn takes_webhooks(self) -> bool {
        matches!(self, Self::WhatsappCloud | Self::Instagram | Self::X | Self::Linkedin)
    }

    pub fn reply_window_ms(self) -> Option<i64> {
        match self {
            Self::WhatsappCloud | Self::Instagram => Some(24 * HOUR_MS),
            Self::WhatsappWeb | Self::Email | Self::X | Self::Linkedin => None,
        }
    }

    pub fn may_answer(self, last_contact: Option<i64>, now: i64) -> bool {
        match self.reply_window_ms() {
            None => true,
            Some(window) => last_contact.is_some_and(|at| now - at < window),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::WhatsappWeb => "WhatsApp",
            Self::Email => "E-mail",
            Self::WhatsappCloud => "WhatsApp (Cloud API)",
            Self::Instagram => "Instagram",
            Self::X => "X",
            Self::Linkedin => "LinkedIn",
        }
    }
}

pub const MAX_CHANNEL_HOOK_BYTES: usize = 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_793_016_000_000;

    #[test]
    fn meta_answers_only_within_a_day_of_the_persons_last_message() {
        for kind in [ChannelKind::WhatsappCloud, ChannelKind::Instagram] {
            assert!(kind.may_answer(Some(NOW - 23 * HOUR_MS), NOW));
            assert!(!kind.may_answer(Some(NOW - 24 * HOUR_MS), NOW));
            assert!(!kind.may_answer(None, NOW), "a person who never wrote");
        }
    }

    #[test]
    fn other_channels_have_no_window() {
        for kind in [ChannelKind::WhatsappWeb, ChannelKind::Email, ChannelKind::X, ChannelKind::Linkedin] {
            assert!(kind.may_answer(None, NOW));
        }
    }

    #[test]
    fn the_official_apis_take_webhooks_and_keep_their_own_ceilings() {
        assert!(!ChannelKind::WhatsappWeb.takes_webhooks());
        assert!(!ChannelKind::Email.takes_webhooks());
        assert!(ChannelKind::WhatsappCloud.takes_webhooks());
        assert_eq!(ChannelKind::Email.limits(), Limits::default());
        assert!(ChannelKind::X.limits().per_day <= 100);
    }

    #[test]
    fn kinds_are_named_in_snake_case() {
        let named = serde_json::to_string(&[ChannelKind::WhatsappCloud, ChannelKind::Linkedin]).ok();
        assert_eq!(named.as_deref(), Some(r#"["whatsapp_cloud","linkedin"]"#));
    }
}
