//! Linux: SIGTERM, SIGINT and SIGHUP quit the app as closing its window
//! does. Chromium installs its own handlers when CEF starts, and they ask
//! CEF's loop to quit, which with an external message pump is not the
//! loop that runs the app: the pages closed and the app stayed. These
//! replace them once CEF has started. A handler only writes a byte to a
//! pipe (all a signal handler may safely do); a thread reads it and wakes
//! the loop with [`Wake::Quit`].
#![allow(unsafe_code)]

use std::sync::atomic::{AtomicI32, Ordering};

use winit::event_loop::EventLoopProxy;

use super::Wake;

static WRITE_END: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(_signal: libc::c_int) {
    let fd = WRITE_END.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: write(2) is async-signal-safe; the buffer outlives the call.
        unsafe { libc::write(fd, (&raw const byte).cast(), 1) };
    }
}

pub fn install(proxy: EventLoopProxy<Wake>) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` has room for the two descriptors pipe(2) writes.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        tracing::warn!(error = %std::io::Error::last_os_error(), "quitting signals not taken over");
        return;
    }
    let [read_end, write_end] = fds;
    WRITE_END.store(write_end, Ordering::Relaxed);
    let listening = std::thread::Builder::new().name("superfik-signals".into()).spawn(move || {
        loop {
            let mut byte = 0u8;
            // SAFETY: reads one byte into a live local from our own pipe.
            let read = unsafe { libc::read(read_end, (&raw mut byte).cast(), 1) };
            if read == 1 {
                tracing::info!("asked to quit by a signal");
                if proxy.send_event(Wake::Quit).is_err() {
                    return;
                }
            } else if read < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            } else {
                return;
            }
        }
    });
    if let Err(error) = listening {
        tracing::warn!(%error, "quitting signals not taken over");
        return;
    }
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: the handler only loads an atomic and calls write(2).
        unsafe { libc::signal(signal, on_signal as *const () as libc::sighandler_t) };
    }
}
