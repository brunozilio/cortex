#![allow(unsafe_code)]

pub fn port_holder(port: u16) -> Option<(u32, String)> {
    let listening = super::capture("lsof", &["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"])?;
    let pid: u32 = String::from_utf8_lossy(&listening).lines().next()?.trim().parse().ok()?;
    let executable = super::capture("ps", &["-o", "comm=", "-p", &pid.to_string()])?;
    Some((pid, String::from_utf8_lossy(&executable).trim().to_owned()))
}

pub fn terminate(pid: u32) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid).map_err(std::io::Error::other)?;
    match unsafe { libc::kill(pid, libc::SIGTERM) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
