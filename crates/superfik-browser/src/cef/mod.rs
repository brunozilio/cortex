//! The real host: CEF in accelerated off-screen mode, through `welding`
//! (its macOS producer, which also serves Linux).
//!
//! CEF starts ahead of the event loop ([`BrowserHost::warm_up`]). On macOS
//! its message-loop work runs from a run-loop timer (`events::WorkTimer`,
//! in the common modes), which fires while winit waits with its handler
//! set: CEF drains the application's event queue while it works, and doing
//! that inside a winit callback (or while no handler is set) loses or
//! replays events. On Linux the pump does that work itself (`events`).
//! [`BrowserHost::pump`], run by the shell between dispatches, sends calls,
//! collects what pages reported and arms the timer while a page is open.

mod agent;
mod automation;
mod calls;
mod convert;
mod events;
mod files;
mod fill;
mod identity;
mod native_keys;
mod opening;
mod page;
mod passkeys;
mod runtime;
mod tabs;
mod waiting;
mod watch;

use std::collections::HashMap;
use std::path::PathBuf;

use superfik_core::BrowserId;
use welding::macos_cef::{MacosCefConfig, PreparedMacosCefProfile};
use welding::{CefRuntime, CefSurfaceProducer, HostWgpuContext};

pub use self::events::spin;
use self::page::Page;
use self::runtime::runtime;
use crate::{BrowserHost, Call, Frame, PageEvent};

const WORK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(8);
const WARM_UP: std::time::Duration = std::time::Duration::from_secs(1);
const CLOSE_PUMPS: usize = 30;

struct Opening {
    config: MacosCefConfig,
    profile: PreparedMacosCefProfile,
    held: Vec<Call>,
    files: files::FileArea,
}

pub struct CefHost {
    root: PathBuf,
    groups: HashMap<BrowserId, tabs::Group>,
    owners: HashMap<BrowserId, BrowserId>,
    timer: Option<events::WorkTimer>,
    runtime: Option<CefRuntime>,
    broken: Option<String>,
    queue: Vec<Call>,
    opening: HashMap<BrowserId, Opening>,
    pages: HashMap<BrowserId, Page>,
    failures: HashMap<BrowserId, String>,
    pending_events: Vec<(BrowserId, PageEvent)>,
    identities: HashMap<crate::Profile, Option<welding::macos_cef::NativeHardwareOverrides>>,
    profile_credentials: std::sync::Arc<dyn crate::profiles::CredentialStore>,
    test_certificate_spki: Option<String>,
    reduced_services: bool,
    context: Option<HostWgpuContext>,
    warming: Option<std::time::Instant>,
    egress: Option<std::net::SocketAddr>,
}

impl std::fmt::Debug for CefHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CefHost")
            .field("root", &self.root)
            .field("pages", &self.pages.len())
            .field("opening", &self.opening.len())
            .finish_non_exhaustive()
    }
}

impl CefHost {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            groups: HashMap::new(),
            owners: HashMap::new(),
            timer: None,
            runtime: None,
            broken: None,
            queue: Vec::new(),
            opening: HashMap::new(),
            pages: HashMap::new(),
            failures: HashMap::new(),
            pending_events: Vec::new(),
            identities: HashMap::new(),
            profile_credentials: std::sync::Arc::new(crate::profiles::KeychainCredentials),
            test_certificate_spki: None,
            reduced_services: true,
            context: None,
            warming: None,
            egress: None,
        }
    }

    pub fn guarded(mut self, proxy: std::net::SocketAddr) -> Self {
        self.egress = Some(proxy);
        self
    }

    pub fn with_profile_credentials(
        mut self,
        credentials: std::sync::Arc<dyn crate::profiles::CredentialStore>,
    ) -> Self {
        self.profile_credentials = credentials;
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_certificate_spki_for_testing(mut self, spki: String) -> Self {
        self.test_certificate_spki = Some(spki);
        self
    }

    #[cfg(debug_assertions)]
    pub fn with_default_services_for_testing(mut self) -> Self {
        self.reduced_services = false;
        self
    }

    fn start(&mut self) -> Option<&CefRuntime> {
        if self.runtime.is_none() && self.broken.is_none() {
            let initialized = (|| -> Result<CefRuntime, Box<dyn std::error::Error>> {
                std::fs::create_dir_all(&self.root)?;
                self.root = self.root.canonicalize()?;
                runtime(&self.root, self.test_certificate_spki.as_deref(), self.reduced_services, self.egress)
            })();
            match initialized {
                Ok(runtime) => {
                    self.runtime = Some(runtime);
                    self.timer = Some(events::WorkTimer::start(WORK_INTERVAL));
                }
                Err(error) => {
                    tracing::error!(%error, "the browser engine could not start");
                    self.broken = Some(format!("The browser engine could not start: {error}"));
                }
            }
        }
        self.runtime.as_ref()
    }
}

impl BrowserHost for CefHost {
    fn warm_up(&mut self) {
        if self.start().is_some() {
            self.warming = Some(std::time::Instant::now());
        }
    }

    fn send(&mut self, call: Call) {
        self.queue.push(call);
    }

    fn pump(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<(BrowserId, PageEvent)> {
        let active = self.is_active();
        if let Some(timer) = &self.timer {
            timer.set_active(active);
            timer.tick();
        }
        if !active {
            return Vec::new();
        }
        for call in std::mem::take(&mut self.queue) {
            self.apply(call);
        }
        if self.runtime.is_none() {
            return std::mem::take(&mut self.pending_events);
        }
        self.finish_opening();
        self.collect_popups();
        let routing = self.routing();
        let context = self.context.get_or_insert_with(|| HostWgpuContext::new(device.clone(), queue.clone()));
        let mut reported = std::mem::take(&mut self.pending_events);
        for (id, page) in &mut self.pages {
            let mut events = Vec::new();
            page.drain(context, &mut events);
            let (root, active) = routing[id];
            reported.extend(
                events
                    .into_iter()
                    .filter(|event| {
                        active || matches!(event, PageEvent::Answered { .. } | PageEvent::LoginSubmitted(_))
                    })
                    .map(|e| (root, e)),
            );
        }
        self.collect_closed_tabs();
        reported.append(&mut self.pending_events);
        reported
    }

    fn frame(&self, id: BrowserId) -> Option<Frame> {
        let frame = self.pages.get(&self.active_tab(id))?.frame.as_ref()?;
        Some(Frame {
            view: frame.view.clone(),
            size: (frame.size.width, frame.size.height),
            format: frame.format,
            generation: frame.generation,
        })
    }

    fn suspend(&mut self) {
        if let Some(timer) = &self.timer {
            timer.set_active(false);
        }
    }

    fn is_active(&self) -> bool {
        let warming = self.warming.is_some_and(|since| since.elapsed() < WARM_UP);
        warming
            || !self.queue.is_empty()
            || !self.opening.is_empty()
            || !self.pages.is_empty()
            || !self.pending_events.is_empty()
    }
}

impl Drop for CefHost {
    fn drop(&mut self) {
        self.timer = None;
        self.collect_popups();
        let Some(runtime) = self.runtime.as_ref() else { return };
        for page in self.pages.values_mut() {
            log(page.producer.close());
        }
        for _ in 0..CLOSE_PUMPS {
            runtime.do_message_loop_work();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.pages.clear();
        self.opening.clear();
    }
}

fn log_ok<E: std::fmt::Display>(result: Result<(), E>) -> bool {
    let ok = result.is_ok();
    log(result);
    ok
}

pub(super) fn log<E: std::fmt::Display>(result: Result<(), E>) {
    if let Err(error) = result {
        tracing::warn!(%error, "browser call failed");
    }
}
