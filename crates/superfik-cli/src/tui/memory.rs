//! `/memory`: list, search, optimize, undo, forgotten and restore.

use superfik_core::command::MemoryCommand;
use superfik_sdk::{Error, Forgotten, Memory, ProjectMemory};

use super::app::{App, Msg};

pub fn run(app: &mut App, command: MemoryCommand) {
    if command == MemoryCommand::Edit {
        app.edit_file = Some(super::config::instructions_file(&app.cwd));
        return;
    }
    let cwd = app.cwd.clone();
    let model = app.model().cloned();
    app.spawn(move |superfik| {
        let memory = superfik.memory(cwd);
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

/// `#note`: the project's memory keeps it, for every later conversation.
pub fn remember(app: &mut App, note: String) {
    let cwd = app.cwd.clone();
    app.spawn(move |superfik| {
        match superfik.memory(cwd).remember(superfik_sdk::MemoryKind::Semantic, &note, &["user"]) {
            Ok(_) => Msg::Notice(format!("Remembered: {note}")),
            Err(error) => Msg::Error(error.to_string()),
        }
    });
}

fn optimize(memory: &ProjectMemory<'_>, model: superfik_sdk::Model) -> Result<String, Error> {
    let before = memory.list()?.len();
    let mut run = memory.optimize(model)?;
    let mut failed = None;
    while let Some(event) = run.next_event() {
        if let superfik_sdk::EngineEvent::Failed(message) = event {
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

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;
    use superfik_native::fake::FakeEngine;
    use superfik_sdk::{EngineEvent, MemoryKind, Model};

    use super::super::app::App;
    use super::super::tests::{press, screen, superfik, type_text, until};

    fn command(app: &mut App, text: &str, shows: &str) {
        type_text(app, text);
        press(app, KeyCode::Enter);
        until(app, |app| screen(app).contains(shows));
    }

    #[test]
    fn memory_is_listed_forgotten_restored_and_optimized() {
        let data = tempfile::tempdir().expect("data");
        let model = Model::new("anthropic:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let superfik = superfik(&engine, data.path());
        let memory = superfik.memory(data.path());
        memory.remember(MemoryKind::Semantic, "Uses wgpu", &[]).expect("remember");
        memory.remember(MemoryKind::Semantic, "Inter is the font", &[]).expect("remember");
        memory.forget(superfik_sdk::MemoryId(0)).expect("forget");
        let mut app = App::new(superfik, data.path().into(), None, None);
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
        let model = Model::new("anthropic:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        command(&mut app, "# tests run with cargo nextest", "Remembered: tests run with cargo nextest");
        let memories = superfik(&engine, data.path()).memory(data.path()).list().expect("list");
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
