//! CEF's subprocesses (renderer, GPU, utility) run this same binary.
//!
//! On macOS CEF launches the `Superfik Helper*.app` bundles, which are the
//! app's own executable linked in again; on Linux it launches the
//! executable itself. `main` calls [`run_if_helper`] first: in a helper it
//! runs CEF's subprocess and returns its exit code.

pub fn is_helper<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().skip(1).any(|a| a.as_ref().starts_with("--type="))
}

pub fn run_if_helper() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if !is_helper(&args) {
        return None;
    }
    Some(run())
}

#[cfg(target_os = "macos")]
fn run() -> i32 {
    let Ok(exe) = std::env::current_exe() else { return 1 };
    let loader = cef::library_loader::LibraryLoader::new(&exe, true);
    if !loader.load() {
        eprintln!("superfik helper: could not load the Chromium Embedded Framework");
        return 1;
    }
    let args = cef::args::Args::new();
    welding::CefRuntime::try_run_subprocess(&args, welding::CefSandboxMode::UnsandboxedTrustedContent).unwrap_or_else(
        |error| {
            eprintln!("superfik helper: {error}");
            1
        },
    )
}

#[cfg(target_os = "linux")]
fn run() -> i32 {
    let args = cef::args::Args::new();
    welding::CefRuntime::try_run_subprocess(&args, welding::CefSandboxMode::UnsandboxedTrustedContent).unwrap_or_else(
        |error| {
            eprintln!("superfik helper: {error}");
            1
        },
    )
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn run() -> i32 {
    eprintln!("superfik helper: no browser engine on this platform");
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn helpers_are_told_apart_by_their_type_switch() {
        assert!(super::is_helper(&["Superfik Helper", "--type=renderer", "--lang=en"]));
        assert!(!super::is_helper(&["superfik", "/repo"]));
        assert!(!super::is_helper(&["--type=renderer"]), "the program name is not an argument");
    }
}
