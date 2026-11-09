//! The trackpad's actuator through Apple's private MultitouchSupport
//! framework: much firmer pulses than AppKit's public patterns, which are
//! faint enough to miss while dragging. Loaded at run time with `dlopen`,
//! so a macOS without it (or a future one that drops it) falls back to
//! AppKit instead of failing to start. Strikes run on a worker thread, so
//! a burst never holds up a frame.
#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::Strike;

type Ref = *const c_void;
type CreateList = unsafe extern "C" fn() -> Ref;
type DeviceId = unsafe extern "C" fn(Ref, *mut u64) -> c_int;
type Create = unsafe extern "C" fn(u64) -> Ref;
type Open = unsafe extern "C" fn(Ref) -> c_int;
type Actuate = unsafe extern "C" fn(Ref, i32, u32, f32, f32) -> c_int;

const RTLD_LAZY: c_int = 1;
const FRAMEWORK: &[u8] = b"/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport\0";

unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: Ref) -> isize;
    fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    fn CFRelease(object: Ref);
}

struct Actuators {
    refs: Vec<Ref>,
    actuate: Actuate,
}

// SAFETY: the actuator refs are CoreFoundation objects used only by the
unsafe impl Send for Actuators {}

impl Actuators {
    fn open() -> Option<Self> {
        // SAFETY: the path is NUL-terminated; a null handle is checked.
        let handle = unsafe { dlopen(FRAMEWORK.as_ptr().cast(), RTLD_LAZY) };
        if handle.is_null() {
            return None;
        }
        let symbol = |name: &[u8]| {
            // SAFETY: `handle` is a live library handle and `name` is NUL-terminated.
            let found = unsafe { dlsym(handle, name.as_ptr().cast()) };
            (!found.is_null()).then_some(found)
        };
        // SAFETY: each symbol has this C signature in MultitouchSupport
        let (create_list, device_id, create, open, actuate) = unsafe {
            (
                std::mem::transmute::<*mut c_void, CreateList>(symbol(b"MTDeviceCreateList\0")?),
                std::mem::transmute::<*mut c_void, DeviceId>(symbol(b"MTDeviceGetDeviceID\0")?),
                std::mem::transmute::<*mut c_void, Create>(symbol(b"MTActuatorCreateFromDeviceID\0")?),
                std::mem::transmute::<*mut c_void, Open>(symbol(b"MTActuatorOpen\0")?),
                std::mem::transmute::<*mut c_void, Actuate>(symbol(b"MTActuatorActuate\0")?),
            )
        };
        let mut refs = Vec::new();
        // SAFETY: the list is a CFArray we own (released below); its values
        unsafe {
            let list = create_list();
            if list.is_null() {
                return None;
            }
            for index in 0..CFArrayGetCount(list) {
                let mut id = 0u64;
                if device_id(CFArrayGetValueAtIndex(list, index), &mut id) != 0 {
                    continue;
                }
                let actuator = create(id);
                if actuator.is_null() {
                    continue;
                }
                if open(actuator) == 0 {
                    refs.push(actuator);
                } else {
                    CFRelease(actuator);
                }
            }
            CFRelease(list);
        }
        if refs.is_empty() {
            tracing::debug!("no multitouch actuator opened");
            return None;
        }
        Some(Self { refs, actuate })
    }

    fn actuate(&self, actuation: i32) {
        for &actuator in &self.refs {
            // SAFETY: `actuator` was opened by `open` and is kept until drop.
            unsafe { (self.actuate)(actuator, actuation, 0, 0.0, 2.0) };
        }
    }

    fn serve(self, strikes: Receiver<Strike>, burst: Duration) {
        while let Ok(mut strike) = strikes.recv() {
            while let Ok(newer) = strikes.try_recv() {
                strike = newer;
            }
            for i in 0..strike.repeats.max(1) {
                if i > 0 {
                    std::thread::sleep(burst);
                }
                self.actuate(strike.actuation);
            }
        }
    }
}

impl Drop for Actuators {
    fn drop(&mut self) {
        for &actuator in &self.refs {
            // SAFETY: we own one reference to each actuator.
            unsafe { CFRelease(actuator) };
        }
    }
}

#[derive(Debug)]
pub struct Multitouch {
    strikes: Sender<Strike>,
}

impl Multitouch {
    pub fn open(burst: f64) -> Option<Self> {
        let actuators = Actuators::open()?;
        let (strikes, receiver) = mpsc::channel();
        let burst = Duration::from_secs_f64(burst.max(0.0));
        std::thread::Builder::new()
            .name("haptics".into())
            .spawn(move || actuators.serve(receiver, burst))
            .map_err(|error| tracing::warn!(%error, "no haptics thread"))
            .ok()?;
        Some(Self { strikes })
    }

    pub fn strike(&self, strike: Strike) {
        let _ = self.strikes.send(strike);
    }
}
