//! Picks the clip to play for a cue. Pure and deterministic.

use crate::{Cue, PackId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Soundboard {
    pack: PackId,
    cursor: [u8; Cue::ALL.len()],
}

impl Default for Soundboard {
    fn default() -> Self {
        Self::new(PackId::default())
    }
}

impl Soundboard {
    pub fn new(pack: PackId) -> Self {
        Self { pack, cursor: [0; Cue::ALL.len()] }
    }

    pub fn pack(&self) -> PackId {
        self.pack
    }

    pub fn set_pack(&mut self, pack: PackId) {
        self.pack = pack;
    }

    pub fn next(&mut self, cue: Cue) -> &'static [u8] {
        let clips = self.pack.pack().clips(cue);
        let slot = &mut self.cursor[cue.index()];
        let clip = clips[*slot as usize % clips.len()];
        *slot = ((*slot as usize + 1) % clips.len()) as u8;
        clip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_rotate_and_wrap() {
        let mut board = Soundboard::default();
        let taps = PackId::Piano.pack().clips(Cue::Tap);
        let played: Vec<_> = (0..taps.len() + 1).map(|_| board.next(Cue::Tap).as_ptr()).collect();
        let expected: Vec<_> = taps.iter().chain(taps.iter().take(1)).map(|c| c.as_ptr()).collect();
        assert_eq!(played, expected);
    }

    #[test]
    fn single_clips_repeat() {
        let mut board = Soundboard::default();
        assert_eq!(board.next(Cue::Button).as_ptr(), board.next(Cue::Button).as_ptr());
    }

    #[test]
    fn rotation_is_per_cue() {
        let mut board = Soundboard::default();
        board.next(Cue::Tap);
        assert_eq!(board.next(Cue::Type).as_ptr(), PackId::Piano.pack().clips(Cue::Type)[0].as_ptr());
    }

    #[test]
    fn switching_packs_changes_the_voice() {
        let mut board = Soundboard::new(PackId::Piano);
        let piano = board.next(Cue::Select);
        board.set_pack(PackId::Sine);
        assert_eq!(board.pack(), PackId::Sine);
        assert_ne!(board.next(Cue::Select), piano);
    }
}
