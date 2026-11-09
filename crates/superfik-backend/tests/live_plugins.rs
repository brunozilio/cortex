//! Each catalog plugin end to end against the real services: install it
//! through `Plugins`, connect an account, brief a run and let the native engine call one of its MCP tools. Run one with
//! `cargo test -p superfik-backend --test live_plugins <name> -- --ignored --nocapture`.
//!
//! - `playwright`: no account.
//! - `github`: the `gh` CLI must be signed in.
//! - `context7`: no account (free tier).
//! - `stripe`, `firecrawl`, `postgres`: set `STRIPE_API_KEY`, `FIRECRAWL_API_KEY`, `POSTGRES_URL`.
//! - `notion`, `sentry`: open the browser to sign in.
//! - `linear`: opens the browser to sign in.

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use superfik_backend::{FilePluginStore, InMemorySecretStore, McpOAuth, Outbox, Plugins, SystemTokenCommand};
use superfik_core::{Connection, Effort, EngineEvent, PluginId, Principal, PromptRequest, RunId, WorkspaceId};
use superfik_engine::AgentEngine;
use superfik_protocol::ServerEvent;
#[path = "support/native_live.rs"]
mod native_live;

const OWNER: Principal = Principal::LocalOwner;

struct Rig {
    plugins: Plugins,
    outbox: Outbox,
    events: crossbeam_channel::Receiver<ServerEvent>,
    _dir: tempfile::TempDir,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().expect("dir");
    let plugins = Plugins::new(
        Arc::new(FilePluginStore::new(dir.path().to_path_buf())),
        Arc::new(InMemorySecretStore::default()),
        Arc::new(McpOAuth::default()),
        Arc::new(SystemTokenCommand),
    );
    let (tx, events) = crossbeam_channel::unbounded();
    Rig { plugins, outbox: Outbox::new(tx, Arc::new(|| {})), events, _dir: dir }
}

impl Rig {
    fn connect(&self, id: &str, label: &str, values: &[(&str, String)]) -> Vec<Connection> {
        let plugin = PluginId(id.into());
        self.plugins.install_and_set_up(&OWNER, Some(&plugin), None, &self.outbox);
        let installed = self.events.recv_timeout(Duration::from_secs(30)).expect("installed");
        let ServerEvent::Plugins { mut connections, .. } = installed else { panic!("install: {installed:?}") };
        if id == "playwright" {
            let set_up = self.events.recv_timeout(Duration::from_secs(600)).expect("set up");
            assert!(matches!(set_up, ServerEvent::Plugins { .. }), "setup: {set_up:?}");
        }
        if connections.is_empty() {
            let values: Vec<(String, String)> = values.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
            self.plugins.connect(&OWNER, &plugin, label, &values, &self.outbox);
            loop {
                match self.events.recv_timeout(Duration::from_secs(300)).expect("answer") {
                    ServerEvent::AuthorizeAt { url, .. } => {
                        println!("signing in at {url}");
                        let _ = Command::new("open").arg(&url).status();
                    }
                    ServerEvent::Plugins { connections: listed, .. } => {
                        connections = listed;
                        break;
                    }
                    other => panic!("connect: {other:?}"),
                }
            }
        }
        connections
    }

    fn run(&self, accounts: Vec<Connection>, prompt: &str) -> Vec<EngineEvent> {
        let request = PromptRequest {
            workspace: WorkspaceId::new(),
            cwd: std::env::temp_dir(),
            prompt: prompt.into(),
            model: native_live::model(),
            effort: Effort::Low,
            safety_check: false,
            repositories: vec![],
            skills: vec![],
            references: vec![],
            notes: vec![],
            resume: None,
            command: None,
            handoff: None,
            memory: None,
            permission: superfik_core::PermissionMode::Bypass,
            connections: accounts,
            plugins: None,
            browser: None,
            credentials: Default::default(),
            disabled_skills: Default::default(),
            output_style: None,
        };
        let briefed = self.plugins.brief(&OWNER, RunId::new(), &request).expect("brief");
        let brief = briefed.plugins.clone().expect("plugins brief");
        let config = brief.mcp_config.clone().expect("mcp config");
        let _ = brief;
        let (sender, events) = std::sync::mpsc::channel();
        let sink = Arc::new(move |_: RunId, event: EngineEvent| {
            let _ = sender.send(event);
        });
        let engine = native_live::engine();
        let mut handle = engine.start(RunId::new(), &briefed, sink).expect("native run");
        let mut stream = Vec::new();
        loop {
            let event = match events.recv_timeout(Duration::from_secs(300)) {
                Ok(event) => event,
                Err(error) => {
                    handle.cancel();
                    panic!("native run did not finish: {error}");
                }
            };
            if let EngineEvent::Asked(ask) = &event {
                handle.cancel();
                panic!("native run requested permission: {ask:?}");
            }
            let terminal = event.is_terminal();
            stream.push(event);
            if terminal {
                break;
            }
        }
        std::fs::remove_file(&config).ok();
        assert!(matches!(stream.last(), Some(EngineEvent::Completed { .. })), "{stream:?}");
        stream
    }
}

fn report(stream: &[EngineEvent]) -> (Vec<String>, String) {
    let mut tools = Vec::new();
    let mut answer = String::new();
    for event in stream {
        match event {
            EngineEvent::ToolStarted(call) if call.name.starts_with("mcp__") => tools.push(call.name.clone()),
            EngineEvent::TextDelta(text) => answer.push_str(text),
            _ => {}
        }
    }
    println!("tools: {tools:?}\nanswer: {answer}");
    (tools, answer)
}

#[test]
#[ignore = "uses a real native provider and npx"]
fn playwright() {
    let rig = rig();
    let accounts = rig.connect("playwright", "", &[]);
    let stream =
        rig.run(accounts, "Open https://example.com in the browser and tell me its main heading, in one line.");
    let (tools, answer) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__playwright__")), "{stream:?}");
    assert!(answer.to_lowercase().contains("example domain"));
}

#[test]
#[ignore = "uses a real native provider against GitHub"]
fn github() {
    let rig = rig();
    let accounts = rig.connect("github", "", &[]);
    let stream = rig.run(
        accounts,
        "Using the GitHub tools, find the login of the authenticated GitHub user. Answer with the login only.",
    );
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__github__")), "{stream:?}");
}

#[test]
#[ignore = "uses a real native provider against Context7 (free tier, no key)"]
fn context7() {
    let rig = rig();
    let accounts = rig.connect("context7", "", &[]);
    let stream = rig.run(
        accounts,
        "Use the Context7 tools to find the library id of the Rust crate serde. Answer with the id only.",
    );
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__context7__")), "{stream:?}");
}

#[test]
#[ignore = "signs in to Linear in the browser and uses a real native provider"]
fn linear() {
    let rig = rig();
    let accounts = rig.connect("linear", "Work", &[]);
    let stream = rig.run(accounts, "Using the Linear tools, list the names of my Linear teams.");
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__linear-work__")), "{stream:?}");
}

#[test]
#[ignore = "reaches the real authorization servers"]
fn oauth_reaches_the_browser() {
    use superfik_backend::{OAuthFlow, OAuthRequest};
    for id in ["linear", "notion", "sentry"] {
        let manifest =
            superfik_plugins::catalog::packages().into_iter().find(|p| p.id() == id).expect("in catalog").manifest;
        let superfik_plugins::Auth::OAuth(spec) = manifest.auth.clone() else { panic!("{id} is not oauth") };
        let request = OAuthRequest { resource: manifest.first_http_url().expect("url").to_owned(), spec };
        let (tx, rx) = crossbeam_channel::bounded::<Result<String, String>>(1);
        std::thread::spawn(move || {
            let sent = tx.clone();
            let mut open = move |url: String| {
                let _ = sent.send(Ok(url));
            };
            if let Err(error) = McpOAuth::default().sign_in(&request, &mut open) {
                let _ = tx.send(Err(error));
            }
        });
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(Ok(url)) => println!("{id}: {url}"),
            Ok(Err(error)) => panic!("{id}: {error}"),
            Err(_) => panic!("{id}: no answer"),
        }
    }
}

#[test]
#[ignore = "signs in to Notion in the browser and uses a real native provider"]
fn notion() {
    let rig = rig();
    let accounts = rig.connect("notion", "Work", &[]);
    let stream =
        rig.run(accounts, "Using the Notion tools, search my workspace and list the titles of up to three pages.");
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__notion-work__")), "{stream:?}");
}

#[test]
#[ignore = "signs in to Sentry in the browser and uses a real native provider"]
fn sentry() {
    let rig = rig();
    let accounts = rig.connect("sentry", "Work", &[]);
    let stream = rig.run(accounts, "Using the Sentry tools, list the names of my Sentry organizations.");
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__sentry-work__")), "{stream:?}");
}

#[test]
#[ignore = "uses a real native provider against Stripe; needs STRIPE_API_KEY (a test-mode restricted key)"]
fn stripe() {
    let key = std::env::var("STRIPE_API_KEY").expect("STRIPE_API_KEY");
    let rig = rig();
    let accounts = rig.connect("stripe", "test", &[("api_key", key)]);
    let stream = rig.run(accounts, "Using the Stripe tools, tell me my Stripe account's balance.");
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__stripe-test__")), "{stream:?}");
}

#[test]
#[ignore = "uses a real native provider against Firecrawl; needs FIRECRAWL_API_KEY"]
fn firecrawl() {
    let key = std::env::var("FIRECRAWL_API_KEY").expect("FIRECRAWL_API_KEY");
    let rig = rig();
    let accounts = rig.connect("firecrawl", "", &[("api_key", key)]);
    let stream = rig.run(accounts, "Using the Firecrawl tools, scrape https://example.com and tell me its heading.");
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__firecrawl__")), "{stream:?}");
}

#[test]
#[ignore = "uses a real native provider against a database; needs POSTGRES_URL"]
fn postgres() {
    let url = std::env::var("POSTGRES_URL").expect("POSTGRES_URL");
    let rig = rig();
    let accounts = rig.connect("postgres", "local", &[("url", url)]);
    let stream = rig.run(accounts, "Using the PostgreSQL tools, list the tables of the database.");
    let (tools, _) = report(&stream);
    assert!(tools.iter().any(|t| t.starts_with("mcp__postgres-local__")), "{stream:?}");
}
