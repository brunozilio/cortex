//! CEF's loop on macOS. CEF's loop work dispatches the application's
//! queued events itself (scrolls, clicks, keys). Done between winit's
//! dispatches, winit has no handler set and drops them; done inside one,
//! CEF's nested run loop replays winit's queued events into the busy
//! handler, which panics. So a run-loop timer does it, the way CEF's own
//! sample does on macOS: it fires while winit runs the loop, with its
//! handler set and idle, so everything CEF dispatches reaches winit.

#![allow(unsafe_code)]

use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes, NSTimer};

pub fn spin(duration: Duration) {
    let until = objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(duration.as_secs_f64());
    NSRunLoop::currentRunLoop().runUntilDate(&until);
}

pub(in crate::cef) struct WorkTimer {
    timer: Retained<NSTimer>,
    active: Rc<Cell<bool>>,
}

impl WorkTimer {
    pub(in crate::cef) fn start(interval: Duration) -> Self {
        let active = Rc::new(Cell::new(false));
        let on = Rc::clone(&active);
        let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
            if on.get() {
                cef::do_message_loop_work();
            }
        });
        // SAFETY: the timer and its block live on the main thread only: it
        let timer = unsafe {
            let timer = NSTimer::timerWithTimeInterval_repeats_block(interval.as_secs_f64(), true, &block);
            NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
            timer
        };
        Self { timer, active }
    }

    pub(in crate::cef) fn set_active(&self, active: bool) {
        self.active.set(active);
    }

    pub(in crate::cef) fn tick(&self) {}
}

impl Drop for WorkTimer {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}
