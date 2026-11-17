#![allow(unsafe_code)]

use std::io;
use std::process::{Child, Command, ExitStatus};

#[derive(Debug)]
pub(crate) struct Tree {
    child: Child,
}

impl Tree {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        std::os::unix::process::CommandExt::process_group(command, 0);
        Ok(Self { child: command.spawn()? })
    }

    pub fn child(&mut self) -> &mut Child {
        &mut self.child
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn has_exited(&mut self) -> bool {
        let pid = libc::id_t::from(self.child.id());
        let mut seen: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let unreaped = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
        let asked = unsafe { libc::waitid(libc::P_PID, pid, &raw mut seen, unreaped) };
        asked != 0 || seen.si_signo == libc::SIGCHLD
    }

    pub fn ask_to_stop(&mut self) -> bool {
        self.signal(libc::SIGTERM);
        true
    }

    pub fn kill(&mut self) {
        self.signal(libc::SIGKILL);
        let _ = self.child.kill();
    }

    fn signal(&self, signal: libc::c_int) {
        if let Ok(group) = i32::try_from(self.child.id())
            && group > 0
        {
            unsafe { libc::kill(-group, signal) };
        }
    }
}

pub fn lost() -> ExitStatus {
    std::os::unix::process::ExitStatusExt::from_raw(libc::SIGKILL)
}

pub fn alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else { return false };
    if pid <= 0 {
        return false;
    }
    let answered = unsafe { libc::kill(pid, 0) };
    answered == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
