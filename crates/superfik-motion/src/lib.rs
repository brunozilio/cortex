//! Animation primitives for Superfik.
//!
//! Everything here is driven by explicit time values (seconds) instead of
//! reading a clock, so every curve is deterministic and unit-testable.

pub mod easing;
pub mod lerp;
pub mod spring;
pub mod transition;

pub use easing::Easing;
pub use lerp::{Lerp, remap, stagger};
pub use spring::{Spring, SpringConfig};
pub use transition::Presence;
