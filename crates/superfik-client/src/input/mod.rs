//! Input to actions, shared by every shell: keys, the pointer, the wheel
//! and the trackpad become `Action`s against the session and the hits of
//! the last frame. Pure: the shell reads the device, these decide.

pub mod controller;
pub mod keys;
pub mod note_input;
pub mod page_keys;
pub mod page_swipe;
pub mod pointer;
pub mod press;
pub mod push_to_talk;
pub mod route;
pub mod scrolls;
pub mod titlebar;
pub mod wheel;
pub mod wheel_steps;
