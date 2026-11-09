//! The app-facing facade: preferences plus a soundboard wired to a player.

use serde::{Deserialize, Serialize};

use crate::{Cue, PackId, Player, Recorder, Silent, Soundboard, Speakers};

pub const GAIN: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundPrefs {
    pub enabled: bool,
    pub pack: PackId,
}

impl Default for SoundPrefs {
    fn default() -> Self {
        Self { enabled: true, pack: PackId::default() }
    }
}

#[derive(Debug)]
pub struct Sounds {
    prefs: SoundPrefs,
    board: Soundboard,
    player: Box<dyn Player>,
}

impl Sounds {
    pub fn open(prefs: SoundPrefs) -> Self {
        let player: Box<dyn Player> = if !prefs.enabled {
            Box::new(Silent)
        } else {
            match Speakers::open() {
                Ok(speakers) => Box::new(speakers),
                Err(error) => {
                    tracing::warn!(?error, "no audio output; sounds are off");
                    Box::new(Silent)
                }
            }
        };
        Self::with_player(prefs, player)
    }

    pub fn silent(prefs: SoundPrefs) -> Self {
        Self::with_player(prefs, Box::new(Silent))
    }

    pub fn recording(prefs: SoundPrefs) -> (Self, Recorder) {
        let recorder = Recorder::default();
        (Self::with_player(prefs, Box::new(recorder.clone())), recorder)
    }

    fn with_player(prefs: SoundPrefs, player: Box<dyn Player>) -> Self {
        Self { prefs, board: Soundboard::new(prefs.pack), player }
    }

    pub fn prefs(&self) -> SoundPrefs {
        self.prefs
    }

    pub fn set_prefs(&mut self, prefs: SoundPrefs) {
        self.set_enabled(prefs.enabled);
        self.set_pack(prefs.pack);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.prefs.enabled = enabled;
    }

    pub fn set_pack(&mut self, pack: PackId) {
        self.prefs.pack = pack;
        self.board.set_pack(pack);
    }

    pub fn play(&mut self, cue: Cue) {
        if !self.prefs.enabled {
            return;
        }
        let clip = self.board.next(cue);
        tracing::trace!(cue = cue.name(), pack = %self.prefs.pack, "play");
        self.player.play(clip, GAIN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_from_the_chosen_pack_and_respects_mute() {
        let (mut sounds, recorder) = Sounds::recording(SoundPrefs { enabled: true, pack: PackId::Sine });
        sounds.play(Cue::Button);
        assert_eq!(recorder.played(), vec![PackId::Sine.pack().clips(Cue::Button)[0]]);
        sounds.set_pack(PackId::Piano);
        sounds.play(Cue::Button);
        assert_eq!(recorder.played()[1], PackId::Piano.pack().clips(Cue::Button)[0]);
        sounds.set_enabled(false);
        sounds.play(Cue::Button);
        assert_eq!(recorder.played().len(), 2);
    }

    #[test]
    fn prefs_round_trip_and_fill_defaults() {
        let prefs: SoundPrefs = serde_json::from_str(r#"{"pack":"sine"}"#).expect("parses");
        assert_eq!(prefs, SoundPrefs { enabled: true, pack: PackId::Sine });
        let empty: SoundPrefs = serde_json::from_str("{}").expect("parses");
        assert_eq!(empty, SoundPrefs::default());
    }
}
