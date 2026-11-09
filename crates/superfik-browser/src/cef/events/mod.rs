//! When CEF does its work.
//!
//! - macOS ([`mac`]): CEF's loop work dispatches the application's queued
//!   events itself, so it runs from a run-loop timer that fires while winit
//!   waits with its handler set.
//! - Linux ([`linux`]): CEF pumps its own GLib loop and never touches
//!   winit's queue, so the shell's pump does the work directly, between
//!   winit's dispatches.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "linux")]
pub(super) use linux::WorkTimer;
#[cfg(target_os = "linux")]
pub use linux::spin;
#[cfg(target_os = "macos")]
pub(super) use mac::WorkTimer;
#[cfg(target_os = "macos")]
pub use mac::spin;
