//! CEF's loop on Linux. With `external_message_pump` CEF runs its own GLib
//! loop inside `do_message_loop_work` and leaves winit's (X11 or Wayland)
//! queue alone, so the work is done right in the shell's pump, which runs
//! between winit's dispatches every few milliseconds while pages are open.

use std::cell::Cell;
use std::time::{Duration, Instant};

pub fn spin(duration: Duration) {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        cef::do_message_loop_work();
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub(in crate::cef) struct WorkTimer {
    active: Cell<bool>,
}

impl WorkTimer {
    pub(in crate::cef) fn start(_interval: Duration) -> Self {
        Self { active: Cell::new(false) }
    }

    pub(in crate::cef) fn set_active(&self, active: bool) {
        self.active.set(active);
    }

    pub(in crate::cef) fn tick(&self) {
        if self.active.get() {
            cef::do_message_loop_work();
        }
    }
}
