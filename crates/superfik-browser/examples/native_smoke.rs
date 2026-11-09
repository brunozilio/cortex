//! Isolated native-engine acceptance test. Requires CEF_PATH to the custom artifact.

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[path = "native_smoke/run.rs"]
mod run;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(code) = superfik_browser::helper::run_if_helper() {
        std::process::exit(code);
    }
    #[cfg(target_os = "macos")]
    superfik_browser::bundle::relaunch_in_bundle()?;
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    return run::main();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("native browser acceptance is not implemented on this platform".into())
}
