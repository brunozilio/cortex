//! Starts the bundled app as its own *responsible process*.
//!
//! macOS attributes privacy checks (Bluetooth, camera…) to the responsible
//! app, which for a child of a terminal is the terminal: its `Info.plist`
//! lacks Superfik's usage strings, so Chromium cannot reach passkeys over
//! Bluetooth. Spawning with responsibility disclaimed (as Chromium and LLDB
//! do) makes the bundle answer for itself. This process stays in the
//! terminal's foreground group, waits and exits with the child's status.

#![allow(unsafe_code)]

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

unsafe extern "C" {
    fn responsibility_spawnattrs_setdisclaim(attrs: *mut libc::posix_spawnattr_t, disclaim: libc::c_int)
    -> libc::c_int;
}

pub fn run_disclaimed(executable: &Path) -> io::Result<i32> {
    let path = cstring(executable.as_os_str().as_bytes())?;
    let args = std::iter::once(Ok(path.clone()))
        .chain(std::env::args_os().skip(1).map(|a| cstring(a.as_bytes())))
        .collect::<io::Result<Vec<_>>>()?;
    let env = std::env::vars_os()
        .map(|(k, v)| cstring(&[k.as_bytes(), b"=", v.as_bytes()].concat()))
        .collect::<io::Result<Vec<_>>>()?;
    let argv = null_terminated(&args);
    let envp = null_terminated(&env);
    // SAFETY: the attributes are initialised before use and destroyed once;
    unsafe {
        let mut attrs: libc::posix_spawnattr_t = std::ptr::null_mut();
        check(libc::posix_spawnattr_init(&mut attrs))?;
        let mut defaults: libc::sigset_t = 0;
        libc::sigfillset(&mut defaults);
        let mut mask: libc::sigset_t = 0;
        libc::sigemptyset(&mut mask);
        let configured = check(libc::posix_spawnattr_setsigdefault(&mut attrs, &defaults))
            .and_then(|()| check(libc::posix_spawnattr_setsigmask(&mut attrs, &mask)))
            .and_then(|()| {
                check(libc::posix_spawnattr_setflags(
                    &mut attrs,
                    (libc::POSIX_SPAWN_SETSIGDEF | libc::POSIX_SPAWN_SETSIGMASK) as libc::c_short,
                ))
            })
            .and_then(|()| check(responsibility_spawnattrs_setdisclaim(&mut attrs, 1)));
        let mut pid: libc::pid_t = 0;
        let spawned = configured.and_then(|()| {
            check(libc::posix_spawn(&mut pid, path.as_ptr(), std::ptr::null(), &attrs, argv.as_ptr(), envp.as_ptr()))
        });
        libc::posix_spawnattr_destroy(&mut attrs);
        spawned?;
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        libc::signal(libc::SIGQUIT, libc::SIG_IGN);
        let mut status = 0;
        while libc::waitpid(pid, &mut status, 0) < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        Ok(exit_code(status))
    }
}

fn exit_code(status: libc::c_int) -> i32 {
    if libc::WIFSIGNALED(status) { 128 + libc::WTERMSIG(status) } else { libc::WEXITSTATUS(status) }
}

fn cstring(bytes: &[u8]) -> io::Result<CString> {
    CString::new(bytes).map_err(io::Error::other)
}

fn null_terminated(strings: &[CString]) -> Vec<*mut libc::c_char> {
    strings.iter().map(|s| s.as_ptr().cast_mut()).chain([std::ptr::null_mut()]).collect()
}

fn check(code: libc::c_int) -> io::Result<()> {
    if code == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(code)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_child_exit_code_or_its_signal_becomes_ours() {
        assert_eq!(exit_code(3 << 8), 3);
        assert_eq!(exit_code(0), 0);
        assert_eq!(exit_code(libc::SIGINT), 130);
    }

    #[test]
    fn a_disclaimed_child_runs_and_reports_its_exit_code() -> io::Result<()> {
        assert_eq!(run_disclaimed(Path::new("/usr/bin/false"))?, 1);
        Ok(())
    }
}
