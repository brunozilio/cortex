//! Superfik desktop entry point.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod logging;
use superfik_client::{state, view};

fn main() {
    if let Some(code) = superfik_browser::helper::run_if_helper() {
        std::process::exit(code);
    }
    let log = logging::writer();
    tracing_subscriber::fmt()
        .with_ansi(!cfg!(target_os = "windows"))
        .with_writer(log)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "superfik=info,warn".into()),
        )
        .init();
    bundle();
    if let Err(error) = app::run() {
        tracing::error!(%error, "superfik exited with an error");
        std::process::exit(1);
    }
}

fn bundle() {
    #[cfg(target_os = "macos")]
    if !std::env::args().any(|a| a == "--snapshot")
        && let Err(error) = superfik_browser::bundle::relaunch_in_bundle()
    {
        tracing::warn!(%error, "running outside an app bundle: browser cards are off");
    }
}
