//! Starting CEF: from the framework inside this app's bundle on macOS,
//! from the files beside the executable on Linux.

use std::path::{Path, PathBuf};

use welding::{CefRuntime, CefRuntimeConfig, CefSandboxMode};

#[cfg(target_os = "macos")]
fn cef_files(exe: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(exe
        .parent()
        .and_then(Path::parent)
        .map(|contents| contents.join("Frameworks"))
        .ok_or("the app is not inside a bundle")?)
}

#[cfg(target_os = "linux")]
fn cef_files(exe: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = exe.parent().ok_or("the executable has no folder")?;
    if dir.join("icudtl.dat").is_file() && dir.join("libcef.so").is_file() {
        return Ok(dir.to_path_buf());
    }
    if dir.file_name().is_some_and(|name| name == "examples" || name == "deps")
        && let Some(profile) = dir.parent()
        && profile.join("icudtl.dat").is_file()
        && profile.join("libcef.so").is_file()
    {
        return Ok(profile.to_path_buf());
    }
    Err(format!("CEF's files are not beside {}", exe.display()).into())
}

pub(super) fn runtime(
    root: &Path,
    certificate_spki: Option<&str>,
    reduced_services: bool,
    proxy: Option<std::net::SocketAddr>,
) -> Result<CefRuntime, Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let frameworks = cef_files(&exe)?;
    std::fs::create_dir_all(root)?;
    let mut config = CefRuntimeConfig::new(frameworks, CefSandboxMode::UnsandboxedTrustedContent);
    config.cache_path = Some(root.to_path_buf());
    config.command_line_switches = crate::services::switches();
    if cfg!(debug_assertions) && !reduced_services {
        config.command_line_switches.retain(|(name, _)| crate::services::ESSENTIAL.contains(&name.as_str()));
    }
    if cfg!(debug_assertions)
        && let Some(spki) = certificate_spki
    {
        config.command_line_switches.push(("ignore-certificate-errors-spki-list".into(), Some(spki.into())));
    }
    if cfg!(debug_assertions) && std::env::var_os("SUPERFIK_BROWSER_SOFTWARE_GL").is_some_and(|v| v == "1") {
        config.command_line_switches.extend(crate::services::software_gl());
    }
    if let Some(proxy) = proxy {
        config.command_line_switches.extend(crate::services::through(proxy));
    }
    Ok(CefRuntime::initialize(config)?)
}
