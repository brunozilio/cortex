//! Opening a web address in the user's browser.
//!
//! Only `http` and `https` addresses are opened: the system's openers also
//! run files and other schemes, and addresses reach us from plugins and
//! servers. On Windows the address goes to `ShellExecuteW` as is; it never
//! passes through `cmd`, which reads `&` as a command separator.

pub fn openable(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")) && !url.chars().any(char::is_control)
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
pub fn command_for(os: &str, url: &str) -> Option<(&'static str, Vec<String>)> {
    match os {
        "windows" => None,
        "macos" => Some(("open", vec![url.to_owned()])),
        _ => Some(("xdg-open", vec![url.to_owned()])),
    }
}

pub fn open(url: &str) {
    if !openable(url) {
        tracing::warn!(%url, "refused to open an address that is not http(s)");
        return;
    }
    if let Err(error) = open_with_system(url) {
        tracing::error!(%error, "could not open the browser");
    }
}

#[cfg(not(target_os = "windows"))]
fn open_with_system(url: &str) -> std::io::Result<()> {
    match command_for(std::env::consts::OS, url) {
        Some((program, args)) => std::process::Command::new(program).args(args).spawn().map(drop),
        None => Err(std::io::Error::other("no opener on this system")),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn open_with_system(url: &str) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide = |text: &str| std::ffi::OsStr::new(text).encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let (verb, file) = (wide("open"), wide(url));
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call;
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize > 32 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_addresses_are_opened() {
        assert!(openable("https://example.com/authorize?challenge=a&redirect=b&state=c"));
        assert!(openable("HTTP://localhost:7420/"));
        assert!(!openable("file:///C:/Windows/System32/calc.exe"));
        assert!(!openable("C:\\Windows\\System32\\calc.exe"));
        assert!(!openable("javascript:alert(1)"));
        assert!(!openable("https://example.com/\r\nnext"));
    }

    #[test]
    fn an_address_with_ampersands_stays_one_argument() {
        let url = "https://example.com/authorize?challenge=a&redirect=http%3A%2F%2F127.0.0.1&state=^c";
        assert_eq!(command_for("macos", url), Some(("open", vec![url.to_owned()])));
        assert_eq!(command_for("linux", url), Some(("xdg-open", vec![url.to_owned()])));
    }

    #[test]
    fn windows_never_goes_through_a_shell_command() {
        assert_eq!(command_for("windows", "https://a.b/?x=1&y=2"), None);
    }
}
