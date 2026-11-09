//! What the system calls a key. CEF rebuilds each key event from its
//! `native_key_code` and character as the platform's own event would carry
//! them, so editing keys work only with the system's codes: the Mac's
//! `kVK_*` ([`mac`]), or the XKB keycode Linux's X11 and Wayland give
//! ([`linux`]).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "linux")]
pub use linux::{function_char, native_code};
#[cfg(target_os = "macos")]
pub use mac::{function_char, native_code};
