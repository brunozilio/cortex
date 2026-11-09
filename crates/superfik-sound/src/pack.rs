//! Embedded sound packs. Every pack ships the same cues from
//! `assets/sounds/<pack>/`, so any of them can be selected at runtime.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Cue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackId {
    #[default]
    Piano,
    Sine,
    #[serde(alias = "ios")]
    Internal,
}

impl PackId {
    pub const ALL: [PackId; 3] = [PackId::Piano, PackId::Sine, PackId::Internal];

    pub fn name(self) -> &'static str {
        match self {
            PackId::Piano => "piano",
            PackId::Sine => "sine",
            PackId::Internal => "internal",
        }
    }

    pub fn pack(self) -> &'static Pack {
        match self {
            PackId::Piano => &PIANO,
            PackId::Sine => &SINE,
            PackId::Internal => &INTERNAL,
        }
    }
}

impl fmt::Display for PackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPack(pub String);

impl fmt::Display for UnknownPack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown sound pack `{}`", self.0)
    }
}

impl std::error::Error for UnknownPack {}

impl FromStr for PackId {
    type Err = UnknownPack;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PackId::ALL
            .iter()
            .copied()
            .find(|p| p.name().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| UnknownPack(s.into()))
    }
}

#[derive(Debug)]
pub struct Pack {
    pub id: PackId,
    button: &'static [&'static [u8]],
    select: &'static [&'static [u8]],
    tap: &'static [&'static [u8]],
    r#type: &'static [&'static [u8]],
    swipe: &'static [&'static [u8]],
    toggle_on: &'static [&'static [u8]],
    toggle_off: &'static [&'static [u8]],
    transition_up: &'static [&'static [u8]],
    transition_down: &'static [&'static [u8]],
    notification: &'static [&'static [u8]],
    caution: &'static [&'static [u8]],
    disabled: &'static [&'static [u8]],
    celebration: &'static [&'static [u8]],
}

impl Pack {
    pub fn clips(&self, cue: Cue) -> &'static [&'static [u8]] {
        match cue {
            Cue::Button => self.button,
            Cue::Select => self.select,
            Cue::Tap => self.tap,
            Cue::Type => self.r#type,
            Cue::Swipe => self.swipe,
            Cue::ToggleOn => self.toggle_on,
            Cue::ToggleOff => self.toggle_off,
            Cue::TransitionUp => self.transition_up,
            Cue::TransitionDown => self.transition_down,
            Cue::Notification => self.notification,
            Cue::Caution => self.caution,
            Cue::Disabled => self.disabled,
            Cue::Celebration => self.celebration,
        }
    }
}

macro_rules! clip {
    ($dir:literal, $file:literal) => {
        include_bytes!(concat!("../../../assets/sounds/", $dir, "/", $file, ".wav"))
    };
}

macro_rules! pack {
    ($id:expr, $dir:literal) => {
        Pack {
            id: $id,
            button: &[clip!($dir, "button")],
            select: &[clip!($dir, "select")],
            tap: &[
                clip!($dir, "tap_01"),
                clip!($dir, "tap_02"),
                clip!($dir, "tap_03"),
                clip!($dir, "tap_04"),
                clip!($dir, "tap_05"),
            ],
            r#type: &[
                clip!($dir, "type_01"),
                clip!($dir, "type_02"),
                clip!($dir, "type_03"),
                clip!($dir, "type_04"),
                clip!($dir, "type_05"),
            ],
            swipe: &[
                clip!($dir, "swipe_01"),
                clip!($dir, "swipe_02"),
                clip!($dir, "swipe_03"),
                clip!($dir, "swipe_04"),
                clip!($dir, "swipe_05"),
            ],
            toggle_on: &[clip!($dir, "toggle_on")],
            toggle_off: &[clip!($dir, "toggle_off")],
            transition_up: &[clip!($dir, "transition_up")],
            transition_down: &[clip!($dir, "transition_down")],
            notification: &[clip!($dir, "notification")],
            caution: &[clip!($dir, "caution")],
            disabled: &[clip!($dir, "disabled")],
            celebration: &[clip!($dir, "celebration")],
        }
    };
}

static PIANO: Pack = pack!(PackId::Piano, "piano");
static SINE: Pack = pack!(PackId::Sine, "sine");

static INTERNAL: Pack = Pack {
    id: PackId::Internal,
    button: &[clip!("internal", "SentMessage")],
    select: &[clip!("internal", "Tink")],
    tap: &[
        clip!("internal", "focus_change_small"),
        clip!("internal", "focus_change_keyboard"),
        clip!("internal", "focus_change_app_icon"),
    ],
    r#type: &[
        clip!("internal", "keyboard_press_normal"),
        clip!("internal", "key_press_click"),
        clip!("internal", "key_press_modifier"),
    ],
    swipe: &[clip!("internal", "shake")],
    toggle_on: &[clip!("internal", "jbl_begin")],
    toggle_off: &[clip!("internal", "jbl_cancel")],
    transition_up: &[clip!("internal", "navigation_push")],
    transition_down: &[clip!("internal", "navigation_pop")],
    notification: &[clip!("internal", "ReceivedMessage")],
    caution: &[clip!("internal", "payment_failure")],
    disabled: &[clip!("internal", "jbl_no_match")],
    celebration: &[clip!("internal", "payment_success")],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pack_voices_every_cue_with_valid_wav() {
        for id in PackId::ALL {
            let pack = id.pack();
            assert_eq!(pack.id, id);
            for cue in Cue::ALL {
                let clips = pack.clips(cue);
                assert!(!clips.is_empty(), "{id}: {} has no clip", cue.name());
                for clip in clips {
                    assert_eq!(&clip[0..4], b"RIFF", "{id}: {} is not RIFF", cue.name());
                    assert_eq!(&clip[8..12], b"WAVE", "{id}: {} is not WAVE", cue.name());
                }
            }
        }
    }

    #[test]
    fn packs_differ() {
        assert_ne!(PackId::Piano.pack().clips(Cue::Button)[0], PackId::Sine.pack().clips(Cue::Button)[0]);
    }

    #[test]
    fn ids_parse_from_their_names() {
        assert_eq!("sine".parse(), Ok(PackId::Sine));
        assert_eq!(" Piano ".parse(), Ok(PackId::Piano));
        assert_eq!(" Internal ".parse(), Ok(PackId::Internal));
        assert_eq!("harp".parse::<PackId>(), Err(UnknownPack("harp".into())));
        assert_eq!(PackId::default(), PackId::Piano);
    }

    #[test]
    fn ids_serialise_lowercase() {
        assert_eq!(serde_json::to_string(&PackId::Sine).ok().as_deref(), Some("\"sine\""));
        assert_eq!(serde_json::to_string(&PackId::Internal).ok().as_deref(), Some("\"internal\""));
        assert_eq!(serde_json::from_str::<PackId>("\"ios\"").ok(), Some(PackId::Internal));
    }
}
