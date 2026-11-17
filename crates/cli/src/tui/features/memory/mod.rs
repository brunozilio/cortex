use app::{Error, Forgotten, Memory, ProjectMemory};
use domain::command::MemoryCommand;

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["memory"],
            about: "List the project's memory, /memory <search>|optimize|undo|forgotten|restore <id>|edit|seed",
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
        app.ui.edit_file = Some(instructions_file(&app.cwd));
        return;
    }
    let seeding = matches!(command, MemoryCommand::Seed | MemoryCommand::SeedKeep(_));
    if seeding && !super::on(app, domain::features::MEMORY_SEED) {
        return app
            .transcript
            .notice(format!("/memory seed is turned off ({}); /features turns it on.", domain::features::MEMORY_SEED));
    }
    let cwd = app.cwd.clone();
    let model = app.model().cloned();
    app.spawn(move |client| {
        let memory = client.memory(cwd);
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
            MemoryCommand::Share(id) => memory.share(id).map(|file| {
                format!("Shared [{}] as {}: commit it and the team's agents recall it too.", id.0, file.display())
            }),
            MemoryCommand::Seed => memory.seed().map(|offered| seed_listing(&offered)),
            MemoryCommand::SeedKeep(picked) => memory.keep_seeds(&picked).map(|all| listing("Kept. Memory", &all)),
            MemoryCommand::Score => memory.score(),
        };
        match answer {
            Ok(text) => Msg::Notice(text),
            Err(error) => Msg::Error(error.to_string()),
        }
    });
}

fn remember(app: &mut App, note: String) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.memory(cwd).remember(app::MemoryKind::Semantic, &note, &["user"]) {
        Ok(_) => Msg::Notice(format!("Remembered: {note}")),
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn optimize(memory: &ProjectMemory<'_>, model: app::Model) -> Result<String, Error> {
    let before = memory.list()?.len();
    let mut run = memory.optimize(model)?;
    let mut failed = None;
    while let Some(event) = run.next_event() {
        if let app::EngineEvent::Failed(message) = event {
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

fn seed_listing(offered: &[app::SeedCandidate]) -> String {
    if offered.is_empty() {
        return "Nothing you asked for in two or more past sessions is left to remember.".to_owned();
    }
    let lines: Vec<String> = offered
        .iter()
        .enumerate()
        .map(|(at, candidate)| format!("  {}. {} ({} sessions)", at + 1, candidate.text, candidate.sessions))
        .collect();
    format!("Said in two or more past sessions (/memory seed keep <n> [<n>…] remembers them):\n{}", lines.join("\n"))
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
    use app::{EngineEvent, MemoryKind, Model};
    use ratatui::crossterm::event::KeyCode;

    use crate::tui::app::App;
    use crate::tui::tests::{client, press, screen, type_text, until};

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
        let client = client(&engine, data.path());
        let memory = client.memory(data.path());
        memory.remember(MemoryKind::Semantic, "Uses wgpu", &[]).expect("remember");
        memory.remember(MemoryKind::Semantic, "Inter is the font", &[]).expect("remember");
        memory.forget(app::MemoryId(0)).expect("forget");
        let mut app = App::new(client, data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        command(&mut app, "/memory", "[1] Inter is the font");
        command(&mut app, "/memory forgotten", "[0] Uses wgpu");
        command(&mut app, "/memory restore 0", "Restored [0] Uses wgpu");
        command(&mut app, "/memory optimize", "Memory optimized: 2 → 2 memories");
        assert!(
            engine.requests().iter().any(|r| r.prompt.text.contains("[1] semantic, from the user: Inter is the font"))
        );
    }

    #[test]
    fn with_no_past_sessions_there_is_nothing_to_seed_and_keeping_needs_an_offer() {
        let data = tempfile::tempdir().expect("data");
        let model = Model::new("keyed:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        command(&mut app, "/memory seed", "Nothing you asked for in two or more past sessions");
        command(&mut app, "/memory seed keep 1", "None of those numbers is offered");
    }

    #[test]
    fn a_hash_note_is_remembered_and_a_bang_command_runs_for_the_next_prompt() {
        let data = tempfile::tempdir().expect("data");
        let model = Model::new("keyed:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        command(&mut app, "# tests run with cargo nextest", "Remembered: tests run with cargo nextest");
        let memories = client(&engine, data.path()).memory(data.path()).list().expect("list");
        assert!(memories.iter().any(|m| m.text == "tests run with cargo nextest"), "{memories:?}");

        command(&mut app, "!echo from the shell", "from the shell");
        until(&mut app, |app| !app.prompt.shell_context.is_empty());
        type_text(&mut app, "what did it say?");
        press(&mut app, KeyCode::Enter);
        until(&mut app, |_| engine.requests().iter().any(|r| r.session.command.is_none()));
        let sent = engine.requests().into_iter().find(|r| r.session.command.is_none()).expect("sent").prompt.text;
        assert!(sent.contains("$ echo from the shell\nfrom the shell") && sent.ends_with("what did it say?"), "{sent}");
    }
}
