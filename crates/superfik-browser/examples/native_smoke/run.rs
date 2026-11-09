use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use superfik_browser::{BrowserHost, BrowserId, Call, PageEvent, Profile};
use superfik_core::{BrowseId, BrowserOp, Locator};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{Window, WindowId};

type Error = Box<dyn std::error::Error>;
mod acceptance;
mod failure;
mod mission;
mod proxy;

#[derive(Default)]
struct Shell(Option<Window>);
impl ApplicationHandler for Shell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.0.is_none() {
            self.0 = event_loop.create_window(Window::default_attributes().with_visible(false)).ok();
        }
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

pub fn main() -> Result<(), Error> {
    if std::env::var_os("SUPERFIK_HOST_FAILURE_SMOKE").is_some() {
        return failure::check();
    }
    let args: Vec<String> = std::env::args().collect();
    if std::env::var_os("SUPERFIK_AGENT_SMOKE").is_some() {
        return mission::main(&args);
    }
    if std::env::var_os("SUPERFIK_PROXY_SMOKE").is_some() {
        return if args.len() >= 3 && args[1].starts_with("proxy-") {
            proxy::child(Path::new(&args[2]), &args[1])
        } else {
            proxy::main()
        };
    }
    if args.len() >= 3 && ["seed", "check"].contains(&args[1].as_str()) {
        return phase(Path::new(&args[2]), &args[1]);
    }
    acceptance::check()
}

fn fixture() -> Result<String, Error> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            if reader.read_line(&mut request).is_err() {
                continue;
            }
            let hints = request.starts_with("GET /hints ");
            let mut values = serde_json::Map::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some((key, value)) = line.split_once(':')
                    && ["device-memory", "sec-ch-device-memory"].contains(&key.to_ascii_lowercase().as_str())
                {
                    values.insert(key.to_ascii_lowercase(), json!(value.trim()));
                }
            }
            let body = if hints {
                Value::Object(values).to_string()
            } else {
                let fixture = include_str!("fixture.html");
                if cfg!(target_os = "linux") {
                    fixture.replace("Menlo-Regular", "DejaVu Sans Mono")
                } else {
                    fixture.into()
                }
            };
            let kind = if hints { "application/json" } else { "text/html" };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nAccept-CH: Device-Memory, Sec-CH-Device-Memory\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = reader.get_mut().write_all(response.as_bytes());
        }
    });
    Ok(format!("http://{address}/"))
}

fn phase(root: &Path, phase: &str) -> Result<(), Error> {
    eprintln!("Native acceptance phase: {phase}");
    let url = fixture()?;
    let mut host = superfik_browser::cef::CefHost::new(root.into());
    host.warm_up();
    let mut event_loop = EventLoop::new()?;
    let mut shell = Shell::default();
    let mut host = host;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let mut names = HashMap::new();
    for name in ["a", "b", "off", "private", "real"] {
        let id = BrowserId::new();
        let profile = if name == "private" { Profile::Ephemeral } else { Profile::Persistent(name.into()) };
        names.insert(id, name);
        host.send(Call::Open { id, url: url.clone(), profile, size: (640, 480), scale: 1.0 });
    }
    let start = Instant::now();
    let mut last = Instant::now();
    let mut results = serde_json::Map::new();
    let mut clicked = HashSet::new();
    while start.elapsed() < Duration::from_secs(90) && results.len() < names.len() {
        event_loop.pump_app_events(Some(Duration::from_millis(8)), &mut shell);
        for (id, event) in host.pump(&device, &queue) {
            if let PageEvent::Answered { answer: Err(error), .. } = &event {
                return Err(format!("{}: {error}", names[&id]).into());
            }
            if let PageEvent::Answered { answer: Ok(answer), .. } = &event
                && let Ok(Value::Object(value)) = serde_json::from_str::<Value>(&answer.text)
                && ["page", "frame", "worker", "hints"].iter().all(|key| value.contains_key(*key))
                && value.get("labReady").and_then(Value::as_bool) == Some(true)
            {
                if value.get("pointer").and_then(Value::as_bool) == Some(true) {
                    results.insert(names[&id].into(), Value::Object(value));
                } else if clicked.insert(id) {
                    host.send(Call::Focus(id, true));
                    host.send(Call::Agent {
                        id,
                        request: BrowseId(format!("{id}-pointer")),
                        op: BrowserOp::Click { target: Locator::Point { x: 240., y: 115. } },
                    });
                }
            }
            if let PageEvent::Failed { error, .. } = event {
                eprintln!("Native acceptance failure ({phase}): {error}");
                return Err(error.into());
            }
        }
        if last.elapsed() > Duration::from_millis(200) {
            for id in names.keys() {
                if !results.contains_key(names[id]) {
                    host.send(Call::Agent {
                        id: *id,
                        request: BrowseId(format!("{id}")),
                        op: BrowserOp::Evaluate { script: "window.result || null".into() },
                    });
                }
            }
            last = Instant::now();
        }
    }
    if results.len() != names.len() {
        return Err(format!("timed out: {results:?}").into());
    }
    std::fs::write(root.join(format!("{phase}.json")), serde_json::to_vec_pretty(&results)?)?;
    eprintln!("Native acceptance observations ({phase}): {results:?}");
    for id in names.keys() {
        host.send(Call::Close(*id));
    }
    let closing = Instant::now();
    while closing.elapsed() < Duration::from_secs(2) {
        event_loop.pump_app_events(Some(Duration::from_millis(8)), &mut shell);
        host.pump(&device, &queue);
    }
    drop(host);
    Ok(())
}
