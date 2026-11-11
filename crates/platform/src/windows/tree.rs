#![allow(unsafe_code)]

use std::io;
use std::process::{Child, Command};

#[derive(Debug)]
pub struct Tree {
    child: Child,
    job: job::Job,
}

impl Tree {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let job = job::Job::new()?;
        let child = command.spawn()?;
        if let Err(error) = job.assign(&child) {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok(Self { child, job })
    }

    pub fn child(&mut self) -> &mut Child {
        &mut self.child
    }

    pub fn kill(&mut self) {
        self.job.terminate();
        let _ = self.child.kill();
    }
}

mod job {
    use std::io;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::process::Child;

    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
        TerminateJobObject,
    };

    #[derive(Debug)]
    pub(super) struct Job(OwnedHandle);

    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub(super) fn new() -> io::Result<Self> {
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Self(unsafe { OwnedHandle::from_raw_handle(handle) });
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = unsafe {
                SetInformationJobObject(
                    job.0.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    u32::try_from(std::mem::size_of_val(&limits)).unwrap_or(u32::MAX),
                )
            };
            if set == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }

        pub(super) fn assign(&self, child: &Child) -> io::Result<()> {
            if unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child.as_raw_handle()) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        pub(super) fn terminate(&self) {
            unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) };
        }
    }
}

#[cfg(test)]
mod windows_tests {
    use std::io::{self, BufRead};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::process::Stdio;

    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};

    #[test]
    fn a_process_tree_can_be_owned_by_a_shared_transport() {
        fn thread_safe<T: Send + Sync>() {}
        thread_safe::<super::Tree>();
    }

    #[test]
    fn terminating_the_job_stops_a_spawned_grandchild() -> io::Result<()> {
        let mut command = super::super::command("powershell.exe");
        command.args([
            "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
            "$child = Start-Process -FilePath $env:ComSpec -ArgumentList '/d /c ping -n 30 127.0.0.1 >nul' -PassThru -WindowStyle Hidden; Write-Output $child.Id; Wait-Process -Id $child.Id",
        ]).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut tree = super::Tree::spawn(&mut command)?;
        let stdout = tree.child().stdout.take().ok_or_else(|| io::Error::other("no stdout"))?;
        let mut line = String::new();
        io::BufReader::new(stdout).read_line(&mut line)?;
        let pid: u32 = line.trim().parse().map_err(io::Error::other)?;
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let child = unsafe { OwnedHandle::from_raw_handle(handle) };
        tree.kill();
        tree.child().wait()?;
        let stopped = unsafe { WaitForSingleObject(child.as_raw_handle(), 5000) };
        assert_eq!(stopped, WAIT_OBJECT_0, "the grandchild outlived its job");
        Ok(())
    }
}
