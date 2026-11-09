//! The emote on the Dock tile. The face mirrors the composer's: it follows
//! the same state, morphs the same way and is drawn with the icon's own
//! colours, but only as often as its motion needs.

use superfik_dock::{Image, Tile};
use superfik_ui::emote::{self, EmoteState};
use superfik_ui::theme::{colors, motion};

const SIZE: u32 = 256;

pub struct DockEmote {
    tile: Box<dyn Tile>,
    enabled: bool,
    drawn: Option<(EmoteState, f64)>,
}

impl DockEmote {
    pub fn new(tile: Box<dyn Tile>, enabled: bool) -> Self {
        Self { tile, enabled, drawn: None }
    }

    pub fn sync(&mut self, state: EmoteState, from: Option<(EmoteState, f32)>, now: f64) {
        if !self.enabled || !due(state, from.is_some_and(|(_, k)| k < 1.0), now, self.drawn) {
            return;
        }
        let frame = emote::frame(state, from, now as f32);
        let rgba = emote::raster(&frame, &colors::ICON_LOOK, SIZE);
        if let Some(image) = Image::new(SIZE, SIZE, rgba) {
            self.tile.set(&image);
        }
        self.drawn = Some((state, now));
    }
}

pub fn due(state: EmoteState, morphing: bool, now: f64, drawn: Option<(EmoteState, f64)>) -> bool {
    let Some((last, at)) = drawn else { return true };
    if last != state {
        return true;
    }
    let interval = if morphing {
        motion::EMOTE_DOCK_LIVE
    } else {
        match state {
            EmoteState::Thinking => motion::EMOTE_DOCK_LIVE,
            EmoteState::Alert | EmoteState::Done => motion::EMOTE_DOCK_CALM,
            EmoteState::Question => motion::EMOTE_DOCK_CALM * 2.0,
            EmoteState::Idle => {
                let t = now as f32;
                let near_blink = [-0.15, 0.0, 0.15].iter().any(|d| emote::blink(t + d) > 0.0);
                if near_blink { motion::EMOTE_DOCK_LIVE } else { return false }
            }
        }
    };
    now - at >= f64::from(interval)
}

#[cfg(test)]
mod tests {
    use superfik_dock::Recorder;

    use super::*;

    #[test]
    fn first_frame_and_state_changes_always_draw() {
        assert!(due(EmoteState::Idle, false, 0.0, None));
        assert!(due(EmoteState::Alert, false, 0.01, Some((EmoteState::Idle, 0.0))));
    }

    #[test]
    fn thinking_redraws_at_the_live_rate() {
        let at = 1.0;
        let live = f64::from(motion::EMOTE_DOCK_LIVE);
        assert!(!due(EmoteState::Thinking, false, at + live * 0.5, Some((EmoteState::Thinking, at))));
        assert!(due(EmoteState::Thinking, false, at + live, Some((EmoteState::Thinking, at))));
    }

    #[test]
    fn idle_only_redraws_around_a_blink() {
        let quiet = 1.0;
        assert_eq!(emote::blink(quiet as f32), 0.0);
        assert!(!due(EmoteState::Idle, false, quiet, Some((EmoteState::Idle, 0.0))));
        let blink = f64::from(motion::EMOTE_BLINK_PERIOD - 1.7 + motion::EMOTE_BLINK * 0.5);
        assert!(due(EmoteState::Idle, false, blink, Some((EmoteState::Idle, 0.0))));
    }

    #[test]
    fn sync_draws_through_the_tile_only_when_due() {
        let mut dock = DockEmote::new(Box::new(Recorder::default()), true);
        dock.sync(EmoteState::Thinking, None, 0.0);
        dock.sync(EmoteState::Thinking, None, 0.01);
        dock.sync(EmoteState::Done, None, 0.02);
        assert_eq!(dock.drawn.map(|(s, _)| s), Some(EmoteState::Done));
        let mut off = DockEmote::new(Box::new(Recorder::default()), false);
        off.sync(EmoteState::Thinking, None, 0.0);
        assert!(off.drawn.is_none());
    }
}
