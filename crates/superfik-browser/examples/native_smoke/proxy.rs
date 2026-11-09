//! Real BrowserHost proxy authentication and fail-closed acceptance.
#[path = "proxy_server.rs"]
mod server;
use super::{Error, Shell};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use superfik_browser::profiles::CredentialStore;
use superfik_browser::{BrowserHost, BrowserId, Call, PageEvent, Profile};
use superfik_core::browser_profiles::{BrowserProfile, ProfileBook, ProfileId, Proxy, ProxyScheme};
use superfik_core::{BrowseId, BrowserOp};
use winit::event_loop::EventLoop;
use winit::platform::pump_events::EventLoopExtPumpEvents;

struct FixtureCredentials {
    wrong: bool,
}
impl CredentialStore for FixtureCredentials {
    fn read(&self, _: ProfileId) -> Result<Option<String>, String> {
        Ok(Some(if self.wrong { "wrong" } else { "lab-password" }.into()))
    }
    fn write(&self, _: ProfileId, _: Option<&str>) -> Result<(), String> {
        Err("fixture is read-only".into())
    }
}

pub fn main() -> Result<(), Error> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let mut proxy = server::ProxyFixture::start(&root)?;
    let closed = std::net::TcpListener::bind("127.0.0.1:0")?;
    let closed_port = closed.local_addr()?.port();
    drop(closed);
    let exe = std::env::current_exe()?;
    for phase in [
        "proxy-valid",
        "proxy-valid-https",
        "proxy-connect",
        "proxy-https-connect",
        "proxy-dead",
        "proxy-https-dead",
        "proxy-wrong-password",
        "proxy-https-wrong-password",
        "proxy-untrusted-https",
    ] {
        let before = proxy.stats()?;
        let path = root.join(phase);
        std::fs::create_dir(&path)?;
        let mut profile = BrowserProfile::new("Proxy fixture".into());
        profile.settings.enabled = true;
        profile.settings.proxy = Some(Proxy {
            scheme: if phase.contains("https") { ProxyScheme::Https } else { ProxyScheme::Http },
            host: "127.0.0.1".into(),
            port: if phase.contains("dead") {
                closed_port
            } else if phase.contains("https") {
                proxy.configuration.https_proxy
            } else {
                proxy.configuration.http_proxy
            },
            username: Some("lab".into()),
        });
        let id = profile.id;
        let mut book = ProfileBook::default();
        book.save(profile)?;
        book.assign("a".into(), Some(id))?;
        superfik_browser::profiles::ProfileStore::new(path.clone()).save(&book)?;
        let status = std::process::Command::new(&exe)
            .arg(phase)
            .arg(&path)
            .env(
                "SUPERFIK_PROXY_TEST_ORIGIN",
                if phase.contains("connect") {
                    &proxy.configuration.https_origin
                } else {
                    &proxy.configuration.http_origin
                },
            )
            .env("SUPERFIK_PROXY_TEST_SPKI", &proxy.configuration.spki)
            .args(["-ApplePersistenceIgnoreState", "YES", "-NSQuitAlwaysKeepsWindows", "NO"])
            .status()?;
        if !status.success() {
            return Err(format!("{phase} failed: {status}; fixture counters: {:?}", proxy.stats()?).into());
        }
        let after = proxy.stats()?;
        if after.direct != 0 || after.udp != 0 {
            return Err(format!("{phase}: direct origin/UDP traffic: {after:?}").into());
        }
        if expected_failure(phase) && after.origin != before.origin {
            return Err(format!("{phase}: failed proxy reached origin: {after:?}").into());
        }
        if !expected_failure(phase) && after.origin < before.origin + 2 {
            return Err(format!("{phase}: missing page/worker proxy traffic: {after:?}").into());
        }
    }
    let stats = proxy.stats()?;
    if stats.authenticated < 4 || stats.denied == 0 || stats.connect < 2 {
        return Err(format!("Missing authenticated proxy/tunnel traffic: {stats:?}").into());
    }
    println!(
        "Proxy acceptance passed: authentication, worker requests, failed HTTP/HTTPS proxy, wrong password and no direct fallback"
    );
    Ok(())
}

fn expected_failure(phase: &str) -> bool {
    phase.contains("dead") || phase.contains("wrong-password") || phase.contains("untrusted")
}

pub fn child(root: &Path, phase: &str) -> Result<(), Error> {
    let url = std::env::var("SUPERFIK_PROXY_TEST_ORIGIN")?;
    let expected_failure = expected_failure(phase);
    let mut host = superfik_browser::cef::CefHost::new(root.into())
        .with_profile_credentials(Arc::new(FixtureCredentials { wrong: phase.contains("wrong-password") }));
    #[cfg(debug_assertions)]
    if !phase.contains("untrusted") {
        host = host.with_certificate_spki_for_testing(std::env::var("SUPERFIK_PROXY_TEST_SPKI")?);
    }
    host.warm_up();
    let mut event_loop = EventLoop::new()?;
    let mut shell = Shell::default();
    let mut host = host;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let id = BrowserId::new();
    host.send(Call::Open { id, url, profile: Profile::Persistent("a".into()), size: (640, 480), scale: 1. });
    let start = Instant::now();
    let mut last = Instant::now();
    let mut result = None;
    let mut failed = false;
    while start.elapsed() < Duration::from_secs(40) && result.is_none() && !failed {
        event_loop.pump_app_events(Some(Duration::from_millis(8)), &mut shell);
        for (_, event) in host.pump(&device, &queue) {
            match event {
                PageEvent::Failed { .. } => failed = true,
                PageEvent::Answered { answer: Ok(answer), .. } => {
                    if let Ok(Value::Object(value)) = serde_json::from_str::<Value>(&answer.text) {
                        result = Some(value);
                    }
                }
                _ => {}
            }
        }
        if last.elapsed() > Duration::from_millis(200) {
            host.send(Call::Agent {
                id,
                request: BrowseId("proxy-fixture".into()),
                op: BrowserOp::Evaluate { script: "window.proxyResult || null".into() },
            });
            last = Instant::now();
        }
    }
    host.send(Call::Close(id));
    drop(shell.0.take());
    let closing = Instant::now();
    while closing.elapsed() < Duration::from_secs(1) {
        event_loop.pump_app_events(Some(Duration::from_millis(8)), &mut shell);
        host.pump(&device, &queue);
    }
    drop(host);
    if expected_failure {
        if !failed {
            return Err(format!("{phase}: expected a network failure").into());
        }
    } else {
        let result = result.ok_or("proxy page did not finish")?;
        if failed || result.get("via") != Some(&Value::Bool(true)) || result.get("worker") != Some(&Value::Bool(true)) {
            return Err(format!("proxy page/worker did not use proxy: {result:?}").into());
        }
        if result.get("candidates").and_then(Value::as_array).is_none_or(|c| !c.is_empty()) {
            return Err(format!("WebRTC emitted a direct ICE candidate: {result:?}").into());
        }
    }
    println!("{phase}: passed");
    Ok(())
}
