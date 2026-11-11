use domain::command::MemoryCommand;
use sdk::{Error, Forgotten, Memory, ProjectMemory};

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["memory"],
            about: "List the project's memory, /memory <search>|optimize|undo|forgotten|restore <id>|edit",
            run: memory,
        }],
        tick: None,
        prefix: Some(('#', |app, note| remember(app, note.unwrap_or_default()))),
        ..Feature::default()
    }
}

fn memory(app: &mut App, argument: Option<String>) {
    match MemoryCommand::parse(argument) {
        Some(command) => run(app, command),
        None => app.transcript.notice("/memory restore needs an id (see /memory forgotten)."),
    }
}

fn run(app: &mut App, command: MemoryCommand) {
    if command == MemoryCommand::Edit {
        app.edit_file = Some(instructions_file(&app.cwd));
        return;
    }
    let cwd = app.cwd.clone();
    let model = app.model().cloned();
    app.spawn(move |sdk| {
        let memory = sdk.memory(cwd);
        let answer = match command {
            MemoryCommand::List => memory.list().map(|all| listing("Memory", &all)),
            MemoryCommand::Search(query) => memory.search(&query).map(|found| {
                let lines: Vec<String> =
                    found.into_iter().map(|(m, score)| format!("  {score:.2}  [{}] {}", m.id.0, m.text)).collect();
                if lines.is_empty() { "Nothing matches.".to_owned() } else { format!("Memory:\n{}", lines.join("\n")) }
            }),
            MemoryCommand::Optimize => match model {
                Some(model) => optimize(&memory, model),
                None => Ok("Pick a model first (/model).".to_owned()),
            },
            MemoryCommand::Undo => memory.undo().map(|all| listing("Memory put back as it was", &all)),
            MemoryCommand::Forgotten => memory.forgotten().map(|forgotten| forgotten_listing(&forgotten)),
            MemoryCommand::Edit => Ok(String::new()),
            MemoryCommand::Restore(id) => memory.restore(id).map(|all| match all.iter().find(|m| m.id == id) {
                Some(m) => format!("Restored [{}] {}", id.0, m.text),
                None => format!("Nothing forgotten has id {} (see /memory forgotten).", id.0),
            }),
        };
        match answer {
            Ok(text) => Msg::Notice(text),
            Err(error) => Msg::Error(error.to_string()),
        }
    });
}

fn remember(app: &mut App, note: String) {
    let cwd = app.cwd.clone();
    app.spawn(move |sdk| match sdk.memory(cwd).remember(sdk::MemoryKind::Semantic, &note, &["user"]) {
        Ok(_) => Msg::Notice(format!("Remembered: {note}")),
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn optimize(memory: &ProjectMemory<'_>, model: sdk::Model) -> Result<String, Error> {
    let before = memory.list()?.len();
    let mut run = memory.optimize(model)?;
    let mut failed = None;
    while let Some(event) = run.next_event() {
        if let sdk::EngineEvent::Failed(message) = event {
            failed = Some(message);
        }
    }
    if let Some(message) = failed {
        return Err(Error::Run(message));
    }
    let after = memory.list()?.len();
    Ok(format!("Memory optimized: {before} → {after} memories. /memory undo puts it back."))
}

fn listing(title: &str, memories: &[Memory]) -> String {
    if memories.is_empty() {
        return "Nothing remembered for this project yet.".to_owned();
    }
    let lines: Vec<String> = memories.iter().map(|m| format!("  [{}] {}", m.id.0, m.text)).collect();
    format!("{title}:\n{}", lines.join("\n"))
}

fn forgotten_listing(forgotten: &[Forgotten]) -> String {
    if forgotten.is_empty() {
        return "Nothing forgotten to restore.".to_owned();
    }
    let lines: Vec<String> = forgotten.iter().map(|f| format!("  [{}] {}", f.memory.id.0, f.memory.text)).collect();
    format!("Forgotten (/memory restore <id>, for 30 days):\n{}", lines.join("\n"))
}

fn instructions_file(cwd: &std::path::Path) -> std::path::PathBuf {
    let root = cwd.join(brand::INSTRUCTIONS);
    let nested = cwd.join(brand::DIR).join(brand::INSTRUCTIONS);
    if !root.exists() && nested.exists() { nested } else { root }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_instructions_file_is_the_one_the_project_has() {
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(super::instructions_file(dir.path()), dir.path().join(brand::INSTRUCTIONS));
        std::fs::create_dir_all(dir.path().join(brand::DIR)).expect("dir");
        std::fs::write(dir.path().join(concat!(brand::dir!(), "/", brand::instructions!())), "x").expect("write");
        assert_eq!(
            super::instructions_file(dir.path()),
            dir.path().join(concat!(brand::dir!(), "/", brand::instructions!()))
        );
    }

    use agent_engine::fake::FakeEngine;
    use ratatui::crossterm::event::KeyCode;
    use sdk::{EngineEvent, MemoryKind, Model};

    use crate::tui::app::App;
    use crate::tui::tests::{press, screen, sdk, type_text, until};

    fn command(app: &mut App, text: &str, shows: &str) {
        type_text(app, text);
        press(app, KeyCode::Enter);
        until(app, |app| screen(app).contains(shows));
    }

    #[test]
    fn memory_is_listed_forgotten_restored_and_optimized() {
        let data = tempfile::tempdir().expect("data");
        let model = Model::new("keyed:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let sdk = sdk(&engine, data.path());
        let memory = sdk.memory(data.path());
        memory.remember(MemoryKind::Semantic, "Uses wgpu", &[]).expect("remember");
        memory.remember(MemoryKind::Semantic, "Inter is the font", &[]).expect("remember");
        memory.forget(sdk::MemoryId(0)).expect("forget");
        let mut app = App::new(sdk, data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        command(&mut app, "/memory", "[1] Inter is the font");
        command(&mut app, "/memory forgotten", "[0] Uses wgpu");
        command(&mut app, "/memory restore 0", "Restored [0] Uses wgpu");
        command(&mut app, "/memory optimize", "Memory optimized: 2 → 2 memories");
        assert!(engine.requests().iter().any(|r| r.prompt.contains("[1] semantic: Inter is the font")));
    }

    #[test]
    fn a_hash_note_is_remembered_and_a_bang_command_runs_for_the_next_prompt() {
        let data = tempfile::tempdir().expect("data");
        let model = Model::new("keyed:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        command(&mut app, "# tests run with cargo nextest", "Remembered: tests run with cargo nextest");
        let memories = sdk(&engine, data.path()).memory(data.path()).list().expect("list");
        assert!(memories.iter().any(|m| m.text == "tests run with cargo nextest"), "{memories:?}");

        command(&mut app, "!echo from the shell", "from the shell");
        until(&mut app, |app| !app.shell_context.is_empty());
        type_text(&mut app, "what did it say?");
        press(&mut app, KeyCode::Enter);
        until(&mut app, |_| engine.requests().iter().any(|r| r.command.is_none()));
        let sent = engine.requests().into_iter().find(|r| r.command.is_none()).expect("sent").prompt;
        assert!(sent.contains("$ echo from the shell\nfrom the shell") && sent.ends_with("what did it say?"), "{sent}");
    }
}
