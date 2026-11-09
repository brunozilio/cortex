//! Browser cards: a real Chromium page on the canvas.
//!
//! A page is rendered off-screen by CEF and handed over as a texture on the
//! app's own wgpu device (IOSurface → Metal on macOS, DMABUF → Vulkan on
//! Linux, no CPU copy; a CPU bitmap where CEF has no GPU), so it zooms,
//! pans and composites like everything else on the canvas.
//!
//! - [`BrowserHost`] is the seam: the app opens pages, forwards input and
//!   takes frames and [`PageEvent`]s back. [`Recorder`] stands in for tests.
//! - [`address`] turns what was typed in the address bar into a URL. Pure.
//! - [`Profile`] keeps each card's cookies and storage apart.
//! - On macOS and Linux, [`cef::CefHost`] is the real host and [`helper`]
//!   answers for CEF's subprocesses; on macOS [`bundle`] turns a bare
//!   `cargo run` binary into the `.app` CEF needs (on Linux CEF's files sit
//!   beside the binary).

pub mod address;
pub mod agent;
pub mod autofill;
pub mod capture;
pub mod devtools;
mod event;
mod host;
pub mod import;
pub mod passkey;
pub mod profiles;
pub mod services;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod singleton;

#[cfg(target_os = "macos")]
pub mod bundle;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod cef;
pub mod helper;

pub use event::PageEvent;
pub use host::{BrowserHost, Call, Frame, Recorder, Unavailable};
pub use superfik_core::BrowserId;
pub use superfik_core::page::{
    EditCommand, KeyInput, KeyPhase, Modifiers, PageCursor, PageInput, PointerButton, Profile,
};
pub use wgpu;

pub fn open_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<(wgpu::Device, wgpu::Queue)> {
    #[cfg(target_os = "linux")]
    {
        if adapter.get_info().backend != wgpu::Backend::Vulkan {
            return None;
        }
        match welding::build_dmabuf_capable_device(adapter, descriptor) {
            Ok(opened) => Some(opened),
            Err(error) => {
                tracing::warn!(%error, "no DMABUF import on this GPU: browser pages paint on the CPU");
                None
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (adapter, descriptor);
        None
    }
}

pub fn system(cache_root: std::path::PathBuf) -> Box<dyn BrowserHost> {
    opened(cache_root, None)
}

pub fn guarded_system(cache_root: std::path::PathBuf, proxy: std::net::SocketAddr) -> Box<dyn BrowserHost> {
    opened(cache_root, Some(proxy))
}

fn opened(cache_root: std::path::PathBuf, proxy: Option<std::net::SocketAddr>) -> Box<dyn BrowserHost> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if singleton::held_elsewhere(&cache_root) {
            tracing::warn!("another Superfik owns {}: browser cards are off", cache_root.display());
            return Box::new(Unavailable::because(
                "Another Superfik instance owns this browser profile. Close it and reopen Superfik.",
            ));
        }
        let host = cef::CefHost::new(cache_root);
        Box::new(match proxy {
            Some(proxy) => host.guarded(proxy),
            None => host,
        })
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (cache_root, proxy);
        Box::new(Unavailable::default())
    }
}
