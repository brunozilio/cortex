//! Agent acceptance through the real BrowserHost; disposable pages and profiles.
use super::{Error, Shell};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::time::{Duration, Instant};
use superfik_browser::{BrowserHost, BrowserId, Call, PageEvent, Profile};
use superfik_core::{BrowseId, BrowserAnswer, BrowserOp, Locator};
use winit::event_loop::EventLoop;
use winit::platform::pump_events::EventLoopExtPumpEvents;
mod capture;
mod clipboard;
mod interactions;
mod passkey;
mod performance;
mod phase;
mod workflows;

pub fn main(args: &[String]) -> Result<(), Error> {
    if args.get(1).is_some_and(|a| a == "agent") {
        return child(Path::new(args.get(2).ok_or("profile directory missing")?));
    }
    if args.get(1).is_some_and(|a| a.starts_with("cookie-")) {
        return cookies(Path::new(args.get(2).ok_or("profile directory missing")?), &args[1]);
    }
    let temporary = tempfile::tempdir()?;
    let phases = if std::env::var_os("SUPERFIK_COOKIE_SMOKE").is_some() {
        vec!["cookie-seed", "cookie-check", "cookie-other", "cookie-private"]
    } else {
        vec!["agent"]
    };
    for phase in phases {
        phase::run(phase, temporary.path())?;
    }
    Ok(())
}

fn cookies(root: &Path, phase: &str) -> Result<(), Error> {
    let url = fixture()?;
    let profile = match phase {
        "cookie-other" => Profile::Persistent("other-mission".into()),
        "cookie-private" => Profile::Ephemeral,
        _ => Profile::Persistent("mission".into()),
    };
    let mut h = Harness::with_profile(root, "about:blank".into(), profile)?;
    h.command(BrowserOp::Navigate { url })?;
    if phase == "cookie-seed" {
        h.evaluate("document.cookie='mission=persist; Max-Age=3600; Path=/';true")?;
    }
    let expected = if matches!(phase, "cookie-other" | "cookie-private") { "false" } else { "true" };
    assert_eq!(
        h.evaluate("document.cookie.includes('mission=persist')")?,
        expected,
        "cookie isolation failed in {phase}"
    );
    eprintln!("Persistent cookie acceptance passed: {phase}");
    Ok(())
}

struct Harness {
    host: superfik_browser::cef::CefHost,
    event_loop: EventLoop<()>,
    shell: Shell,
    device: wgpu::Device,
    queue: wgpu::Queue,
    page: BrowserId,
    sequence: u64,
    submissions: Vec<superfik_browser::capture::Submitted>,
    passkeys: Vec<superfik_browser::passkey::PasskeyAsk>,
}

impl Harness {
    fn new(root: &Path, url: String) -> Result<Self, Error> {
        Self::with_profile(root, url, Profile::Persistent("mission".into()))
    }
    fn with_profile(root: &Path, url: String, profile: Profile) -> Result<Self, Error> {
        let mut host = superfik_browser::cef::CefHost::new(root.into());
        #[cfg(debug_assertions)]
        if std::env::var("SUPERFIK_PERF_POLICY").as_deref() == Ok("default") {
            host = host.with_default_services_for_testing();
        }
        host.warm_up();
        let event_loop = EventLoop::new()?;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        let page = BrowserId::new();
        host.send(Call::Open { id: page, url, profile, size: (900, 700), scale: 1. });
        Ok(Self {
            host,
            event_loop,
            shell: Shell::default(),
            device,
            queue,
            page,
            sequence: 0,
            submissions: Vec::new(),
            passkeys: Vec::new(),
        })
    }

    fn command(&mut self, op: BrowserOp) -> Result<BrowserAnswer, Error> {
        self.sequence += 1;
        let request = BrowseId(format!("mission-{}", self.sequence));
        self.host.send(Call::Agent { id: self.page, request: request.clone(), op });
        let deadline = Instant::now() + Duration::from_secs(40);
        while Instant::now() < deadline {
            self.event_loop.pump_app_events(Some(Duration::from_millis(8)), &mut self.shell);
            for (_, event) in self.host.pump(&self.device, &self.queue) {
                if let PageEvent::LoginSubmitted(submitted) = &event {
                    self.submissions.push(submitted.clone());
                }
                if let PageEvent::PasskeyRequested(ask) = &event {
                    self.passkeys.push(ask.clone());
                }
                if let PageEvent::Answered { request: answer_id, answer } = event
                    && answer_id == request
                {
                    return answer.map_err(Into::into);
                }
            }
        }
        Err(format!("no answer for {}", request.0).into())
    }

    fn evaluate(&mut self, expression: &str) -> Result<String, Error> {
        Ok(self.command(BrowserOp::Evaluate { script: expression.into() })?.text)
    }
}

fn reference(snapshot: &str, name: &str) -> Result<u32, Error> {
    let end = snapshot.find(name).ok_or_else(|| format!("missing {name}: {snapshot}"))?;
    let prefix = &snapshot[..end];
    let start = prefix.rfind("[ref=").ok_or("missing ref")? + 5;
    Ok(prefix[start..].split(|c: char| !c.is_ascii_digit()).next().ok_or("bad ref")?.parse()?)
}

fn fixture() -> Result<String, Error> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer);
                let request = String::from_utf8_lossy(&buffer);
                if request.starts_with("GET /download ") {
                    let body = "downloaded mission bytes";
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=report.txt\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    return;
                }
                if request.starts_with("GET /never ") {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 100000\r\n\r\n<title>Still loading</title>"
                    );
                    std::thread::sleep(Duration::from_secs(25));
                    return;
                }
                let body = if request.starts_with("GET /oauth ") {
                    "<title>OAuth fixture</title><button onclick=\"opener.postMessage('oauth-success','*');window.close()\">Finish sign in</button>".into()
                } else if request.starts_with("GET /frame ") {
                    "<button onclick=\"parent.postMessage('frame-clicked','*')\">Frame action</button>".into()
                } else {
                    include_str!("mission.html")
                        .replace("FRAME_URL", &format!("http://localhost:{}/frame", address.port()))
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    Ok(format!("http://{address}/"))
}

fn child(root: &Path) -> Result<(), Error> {
    if std::env::var_os("SUPERFIK_PERF_SMOKE").is_some() {
        eprintln!("SUPERFIK_PERF pid={} phase=startup state=begin", std::process::id());
    }
    let url = fixture()?;
    let mut h = Harness::new(root, "about:blank".into())?;
    let hanging = format!("{url}never");
    if std::env::var_os("SUPERFIK_CLOSE_SMOKE").is_some() {
        return workflows::close_while_opening(&mut h);
    }
    h.command(BrowserOp::Navigate { url })?;
    if std::env::var_os("SUPERFIK_PERF_SMOKE").is_some() {
        return performance::run(&mut h);
    }
    if std::env::var_os("SUPERFIK_CLIPBOARD_SMOKE").is_some() {
        return clipboard::check(&mut h);
    }
    if std::env::var_os("SUPERFIK_CAPTURE_SMOKE").is_some() {
        return capture::check(&mut h);
    }
    if std::env::var_os("SUPERFIK_PASSKEY_SMOKE").is_some() {
        return passkey::check(&mut h);
    }
    if std::env::var_os("SUPERFIK_FILL_SMOKE").is_some() {
        return workflows::existing_vault_fill(&mut h);
    }
    if std::env::var_os("SUPERFIK_NAVIGATION_SMOKE").is_some() {
        let error =
            h.command(BrowserOp::Navigate { url: hanging }).expect_err("unfinished navigation must not report success");
        assert!(error.to_string().contains("timed out"), "{error}");
        eprintln!("Navigation timeout acceptance passed");
        return Ok(());
    }
    if std::env::var_os("SUPERFIK_OAUTH_SMOKE").is_some() {
        return workflows::oauth(&mut h);
    }
    if std::env::var_os("SUPERFIK_FILES_SMOKE").is_some() {
        return workflows::files(&mut h);
    }
    if std::env::var_os("SUPERFIK_DIALOG_SMOKE").is_some() {
        return workflows::dialogs(&mut h);
    }
    if std::env::var_os("SUPERFIK_WAIT_SMOKE").is_some() {
        return waits(&mut h);
    }
    if std::env::var_os("SUPERFIK_INTERACTION_SMOKE").is_some() {
        return interactions::check(&mut h);
    }
    let first = h.command(BrowserOp::Snapshot)?.text;
    assert!(first.contains("Shadow action"), "closed shadow control absent: {first}");
    assert!(first.contains("Frame action"), "cross-origin frame absent: {first}");
    assert!(!first.contains("do-not-expose-password"), "password leaked");
    let stable = reference(&first, "Stable action")?;
    h.evaluate(
        "document.body.prepend(Object.assign(document.createElement('button'),{textContent:'Inserted'})); true",
    )?;
    let second = h.command(BrowserOp::Snapshot)?.text;
    assert_eq!(reference(&second, "Stable action")?, stable, "refs changed after unrelated insertion");
    h.command(BrowserOp::Click { target: Locator::Ref(stable) })?;
    assert_eq!(h.evaluate("window.clicked")?, "\"stable\"");
    let shadow = reference(&second, "Shadow action")?;
    h.command(BrowserOp::Click { target: Locator::Ref(shadow) })?;
    assert_eq!(h.evaluate("window.clicked")?, "\"shadow\"");
    h.evaluate("document.getElementById('stable').remove(); true")?;
    assert!(h.command(BrowserOp::Click { target: Locator::Ref(stable) }).is_err(), "detached ref silently reused");
    eprintln!("Agent acceptance passed: semantic snapshot, stable refs, closed shadow and stale-node rejection");
    Ok(())
}

fn waits(h: &mut Harness) -> Result<(), Error> {
    h.evaluate("setTimeout(()=>document.body.append('Eventually ready'),450); true")?;
    let op: BrowserOp = serde_json::from_value(
        serde_json::json!({"wait":{"text":"Eventually ready","url":null,"reference":null,"hidden":false,"timeout_ms":2000}}),
    )?;
    h.command(op)?;
    let never: BrowserOp = serde_json::from_value(
        serde_json::json!({"wait":{"text":"Never appears","url":null,"reference":null,"hidden":false,"timeout_ms":350}}),
    )?;
    let start = Instant::now();
    let failure = h.command(never).expect_err("unmet condition must time out");
    assert!(failure.to_string().contains("timed out"), "{failure}");
    assert!(start.elapsed() >= Duration::from_millis(300));
    assert_eq!(h.evaluate("new Promise(resolve=>setTimeout(()=>resolve('awaited'),80))")?, "\"awaited\"");
    eprintln!("Agent wait acceptance passed: delayed SPA content, explicit timeout and awaited evaluation");
    Ok(())
}
