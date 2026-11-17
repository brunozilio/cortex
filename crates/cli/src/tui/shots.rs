use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model};
use domain::events::TaskItem;
use domain::{AgentAsk, AskId, AskKind, SessionId, ToolCall};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::motion::Motion;
use super::shots_html::{DARK, LIGHT, Palette, page};
use super::tests::{client, press, type_text, until};
use super::theme::{self, Background, Theme};
use super::view;

const SIZES: [(u16, u16); 2] = [(80, 24), (120, 40)];

struct Scene {
    name: &'static str,
    events: Vec<EngineEvent>,
    prompt: Option<&'static str>,
    typed: Option<&'static str>,
    settle: fn(&App) -> bool,
}

fn ask(subject: &str) -> EngineEvent {
    EngineEvent::Asked(AgentAsk {
        id: AskId("a".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: subject.into(), rememberable: true, rule: None },
    })
}

fn result(id: &str, output: &str) -> EngineEvent {
    EngineEvent::ToolResult { id: id.into(), output: output.into(), failed: false }
}

fn tool(id: &str, name: &str, input: &str) -> [EngineEvent; 3] {
    let output = match name {
        "Read" => "Read 42 lines",
        "Grep" => "Found 3 matches in 2 files",
        _ => "",
    };
    [
        EngineEvent::ToolStarted(ToolCall::new(id, name, input)),
        result(id, output),
        EngineEvent::ToolFinished { id: id.into() },
    ]
}

fn edit() -> EngineEvent {
    EngineEvent::ToolResult {
        id: "t2".into(),
        output: "Edited src/main.rs · Added 2 lines, removed 1\n   12 -     let port = 3000;\n   12 +     let port = env_port();\n   13 +     log(port);\n   14       serve(port);".into(),
        failed: false,
    }
}

const ANSWER: &str = "The server now reads its port from `PORT` and falls back to **3000**.\n\n## What changed\n\n- `src/main.rs` calls `env_port()` before binding\n- `src/config.rs` parses the variable and rejects values above 65535\n- a test covers the fallback\n\n```rust\nfn env_port() -> u16 {\n    std::env::var(\"PORT\").ok().and_then(|p| p.parse().ok()).unwrap_or(3000)\n}\n```\n\n`cargo test` passes, including the new fallback test.";

fn tasks() -> EngineEvent {
    let item = |id: &str, subject: &str, status: &str| TaskItem {
        id: id.into(),
        subject: subject.into(),
        status: status.into(),
    };
    EngineEvent::Tasks(vec![
        item("1", "Read how the server binds", "completed"),
        item("2", "Parse PORT in config", "in_progress"),
        item("3", "Cover the fallback with a test", "pending"),
    ])
}

fn scenes() -> Vec<Scene> {
    let started = || EngineEvent::SessionStarted(SessionId("s".into()));
    let mut asking =
        vec![started(), EngineEvent::ThinkingDelta("The port is hardcoded; read it from the environment.".into())];
    asking.extend(tool("t1", "Read", "src/main.rs"));
    asking.push(EngineEvent::TextDelta("I'll make the **port** configurable with `PORT`.".into()));
    asking.push(EngineEvent::ToolStarted(ToolCall::new("t2", "Edit", "src/main.rs")));
    asking.push(edit());
    asking.push(EngineEvent::ToolFinished { id: "t2".into() });
    asking.push(ask("cargo test -p server"));
    let mut working = vec![started(), tasks()];
    working.extend(tool("t1", "Read", "src/main.rs"));
    working.extend(tool("t3", "Grep", "\"bind(\" in src"));
    working.push(EngineEvent::ToolStarted(ToolCall::new("t4", "Bash", "cargo test -p server")));
    working.push(EngineEvent::ToolProgress { id: "t4".into(), line: "Compiling server v0.4.0 (crates/server)".into() });
    let mut answered = vec![started()];
    answered.extend(tool("t1", "Read", "src/main.rs"));
    answered.push(EngineEvent::ToolStarted(ToolCall::new("t2", "Edit", "src/main.rs")));
    answered.push(edit());
    answered.push(EngineEvent::ToolFinished { id: "t2".into() });
    answered.push(EngineEvent::ToolStarted(ToolCall::new("t3", "Bash", "cargo test")));
    answered.push(result("t3", "test result: ok. 12 passed; 0 failed"));
    answered.push(EngineEvent::ToolFinished { id: "t3".into() });
    answered.push(EngineEvent::TextDelta(ANSWER.into()));
    answered.push(EngineEvent::Completed { duration_ms: 48_000, cost_usd: Some(0.42) });
    let mut failed = vec![started()];
    failed.extend(tool("t1", "Read", "src/main.rs"));
    failed.push(EngineEvent::Failure(domain::FailureKind::Unavailable));
    failed.push(EngineEvent::Failed("rate limited: the provider is overloaded".into()));
    vec![
        Scene { name: "S01-welcome", events: Vec::new(), prompt: None, typed: None, settle: |_| true },
        Scene {
            name: "S02-ask",
            events: asking,
            prompt: Some("make the port configurable"),
            typed: None,
            settle: |app| app.ask.is_some(),
        },
        Scene {
            name: "S03-working",
            events: working,
            prompt: Some("make the port configurable"),
            typed: None,
            settle: |app| app.transcript.entries.len() > 3,
        },
        Scene {
            name: "S04-answer",
            events: answered,
            prompt: Some("make the port configurable"),
            typed: None,
            settle: |app| !app.busy(),
        },
        Scene { name: "S05-commands", events: Vec::new(), prompt: None, typed: Some("/"), settle: |_| true },
        Scene {
            name: "S06-failed",
            events: failed,
            prompt: Some("make the port configurable"),
            typed: None,
            settle: |app| !app.busy(),
        },
    ]
}

fn build(scene: &Scene, data: &Path) -> App {
    let model = Model::new("keyed:opus", "Opus 5.5");
    let engine = FakeEngine::new(vec![model], scene.events.clone()).with_delay(Duration::from_millis(2));
    let mut app = App::new(client(&engine, data), data.into(), None, None);
    app.ui.motion = std::cell::RefCell::new(Motion::new(false));
    until(&mut app, |app| app.session.is_some());
    app.cwd = PathBuf::from("~/code/shop-api");
    if let Some(prompt) = scene.prompt {
        type_text(&mut app, prompt);
        press(&mut app, KeyCode::Enter);
        until(&mut app, scene.settle);
        std::thread::sleep(Duration::from_millis(80));
        while let Ok(msg) = app.rx.try_recv() {
            app.message(msg);
        }
    }
    if let Some(typed) = scene.typed {
        type_text(&mut app, typed);
    }
    app
}

fn render(app: &App, (width, height): (u16, u16), palette: &Palette, truecolor: bool) -> String {
    let background = if palette.name == LIGHT.name { Background::Light } else { Background::Dark };
    theme::set(Theme { background, truecolor });
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| view::draw(frame, app)).expect("draw");
    let position = terminal.get_cursor_position().ok().map(|p| (p.x, p.y)).filter(|&p| p != (0, 0));
    page(terminal.backend().buffer(), palette, position)
}

#[test]
#[ignore = "writes design stills to SHOTS_DIR"]
fn stills() {
    let Some(out) = std::env::var_os("SHOTS_DIR").map(PathBuf::from) else { return };
    std::fs::create_dir_all(&out).expect("dir");
    for scene in scenes() {
        let data = tempfile::tempdir().expect("data");
        let app = build(&scene, data.path());
        for size in SIZES {
            for palette in [DARK, LIGHT] {
                let file = out.join(format!("{}-{}x{}-{}.html", scene.name, size.0, size.1, palette.name));
                std::fs::write(file, render(&app, size, &palette, true)).expect("write");
            }
        }
        let file = out.join(format!("{}-120x40-dark256.html", scene.name));
        std::fs::write(file, render(&app, (120, 40), &DARK, false)).expect("write");
    }
}
