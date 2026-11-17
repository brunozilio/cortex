use std::time::{Duration, Instant};

use domain::provider::{PortHolder, app_name};

const RELEASE_WAIT: Duration = Duration::from_secs(5);

pub trait Ports: Send + Sync {
    fn holder(&self, port: u16) -> Option<PortHolder>;
    fn close(&self, port: u16, pid: u32) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemPorts;

impl Ports for SystemPorts {
    fn holder(&self, port: u16) -> Option<PortHolder> {
        let (pid, executable) = platform::port_holder(port)?;
        Some(PortHolder { pid, app: app_name(&executable) })
    }

    fn close(&self, port: u16, pid: u32) -> Result<(), String> {
        if pid == std::process::id() {
            return Err(format!("port {port} is held by this very session, which will not close itself"));
        }
        platform::terminate(pid).map_err(|error| format!("could not close PID {pid}: {error}"))?;
        let started = Instant::now();
        while self.holder(port).is_some_and(|holder| holder.pid == pid) {
            if started.elapsed() > RELEASE_WAIT {
                return Err(format!("PID {pid} did not let go of port {port}; close it yourself and try again"));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
}
