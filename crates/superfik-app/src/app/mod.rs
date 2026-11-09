//! The application shell: window, event loop, rendering and effects.
//! It wires the pure pieces (state, view) to the outside world.

mod browser_icons;
mod clipboard;
mod connect;
mod cookie_clearing;
mod dock;
mod effects;
mod images;
mod launch;
mod login_fill;
mod login_seed;
mod logins;
mod managed;
mod menu;
mod page_frames;
mod pages;
mod profile_list;
mod server_join;
mod settings;
mod shell;
mod snapshot;
mod unlock;
mod update;
mod upload;
mod url;
mod voice;
mod window;

use superfik_client::input::{controller, keys as input, page_keys, page_swipe, pointer, titlebar};

pub use shell::run;

fn platform() -> superfik_client::state::Platform {
    if cfg!(target_os = "windows") {
        superfik_client::state::Platform::WINDOWS
    } else if cfg!(target_os = "macos") {
        superfik_client::state::Platform::DESKTOP
    } else {
        superfik_client::state::Platform::LINUX
    }
}
