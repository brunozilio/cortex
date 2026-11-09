//! Haptic feedback on the trackpad.
//!
//! A [`Haptic`] names *what the hand should feel* (a node lifting off, a
//! node meeting an edge, edges lining up, a slider stepping, a gesture
//! passing its point of no return). Each maps to one of the platform's
//! [`Pattern`]s. [`Haptics`] is the app-facing facade: an on/off switch and
//! a throttle so pulses never buzz, driven by explicit time and tested.
//! Only [`Trackpad`] touches the hardware, behind the [`Actuator`] trait.
//!
//! On macOS the trackpad's own actuator is struck directly (firm and
//! reliable); AppKit's faint public patterns are the fallback. Machines
//! without a Force Touch trackpad simply feel nothing.

mod actuator;
mod haptic;
mod haptics;
#[cfg(target_os = "macos")]
mod multitouch;

pub use actuator::{Actuator, Recorder, Still, Trackpad};
pub use haptic::{Haptic, Pattern, Strike};
pub use haptics::Haptics;
