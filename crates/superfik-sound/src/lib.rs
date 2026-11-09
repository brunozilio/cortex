//! UI sound effects.
//!
//! A [`Cue`] names *what happened* (a button press, a toggle, a finished
//! run). A [`Pack`] maps every cue to one or more embedded WAV clips; packs
//! share the same cue set, so switching [`PackId`] changes the character
//! of the whole app at once. The [`Soundboard`] is pure: it picks the next
//! clip for a cue (rotating through variants) and is unit-tested. Only
//! [`Speakers`] touches the audio device, behind the [`Player`] trait.

mod board;
mod cue;
mod pack;
mod player;
mod sounds;

pub use board::Soundboard;
pub use cue::Cue;
pub use pack::{Pack, PackId};
pub use player::{Player, Recorder, Silent, Speakers};
pub use sounds::{SoundPrefs, Sounds};
