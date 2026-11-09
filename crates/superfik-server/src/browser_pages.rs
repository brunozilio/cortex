//! The server's browser (ADR 0010): browser cards' pages run here and
//! stream to clients. On macOS the server runs CEF off-screen from its own
//! development bundle (`Superfik Server.app`, like the app's) on its main
//! thread, while it serves from another; on Linux from CEF's files beside
//! the binary, with Ozone's headless platform when there is no display.
//! Elsewhere, and with `SUPERFIK_PAGES=off`, it has no browser and pages
//! fail to open.

use std::path::Path;
use std::sync::Arc;

use superfik_backend::PageService;
use superfik_pages::PageLoop;

pub fn wanted(setting: Option<&str>) -> bool {
    cfg!(any(target_os = "macos", target_os = "linux"))
        && !matches!(setting.map(str::trim), Some("off" | "0" | "false"))
}

pub fn prepare() -> Option<i32> {
    if let Some(code) = superfik_browser::helper::run_if_helper() {
        return Some(code);
    }
    #[cfg(target_os = "macos")]
    if let Err(error) = superfik_browser::bundle::relaunch_in_bundle_named(superfik_browser::bundle::SERVER_NAME) {
        tracing::warn!(%error, "running outside a bundle: pages will not open");
    }
    None
}

pub fn start(data: &Path) -> (Arc<dyn PageService>, PageLoop) {
    let mut gpu = superfik_pages::Gpu::headless();
    if gpu.is_none() {
        tracing::warn!("no GPU: pages will not open");
    }
    let host = match superfik_pages::egress::start() {
        Ok(egress) => superfik_browser::guarded_system(data.join("pages"), egress.address()),
        Err(error) => {
            tracing::error!(%error, "no egress proxy: pages will not open");
            gpu = None;
            Box::new(superfik_browser::Unavailable::because("No egress proxy: pages cannot open."))
        }
    };
    let (service, page_loop) = superfik_pages::pages(host, gpu);
    (Arc::new(service), page_loop)
}

#[cfg(test)]
mod tests {
    #[test]
    fn pages_run_on_macos_and_linux_unless_turned_off() {
        assert_eq!(super::wanted(None), cfg!(any(target_os = "macos", target_os = "linux")));
        assert!(!super::wanted(Some("off")));
    }
}
