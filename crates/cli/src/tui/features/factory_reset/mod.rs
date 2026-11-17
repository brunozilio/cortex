use std::path::{Path, PathBuf};

use super::{Feature, Slash};
use crate::tui::app::App;
use crate::tui::ask::Pending;

const NAME: &str = "factory_reset";

#[derive(Default)]
struct Asked(Option<Vec<PathBuf>>);

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["factory-reset"],
            about: concat!(
                "Erase everything ",
                brand::name!(),
                " keeps on this machine (sessions, memory, logins, settings) and quit; it stays installed"
            ),
            run,
        }],
        answer: Some(answered),
        ..Feature::default()
    }
}

fn run(app: &mut App, _argument: Option<String>) {
    let found = existing(targets(platform_dirs()));
    guarded(app, found);
}

fn guarded(app: &mut App, found: Vec<PathBuf>) {
    if app.busy() {
        return app.transcript.notice("Wait for the agent to finish (Esc interrupts it) before a factory reset.");
    }
    offer(app, found);
}

fn offer(app: &mut App, found: Vec<PathBuf>) {
    if found.is_empty() {
        return app.transcript.notice(format!("{} keeps nothing on this machine yet.", brand::NAME));
    }
    let folders: Vec<String> = found.iter().map(|dir| dir.display().to_string()).collect();
    if let Some(asked) = app.features.get::<Asked>(NAME) {
        asked.0 = Some(found);
    }
    app.ask = Some(Pending::local(domain::factory_reset::ask(brand::NAME, &folders)));
}

fn answered(app: &mut App, decision: &app::Decision) -> bool {
    let mine = app.ask.as_ref().is_some_and(|pending| pending.is_local(domain::factory_reset::ASK));
    let found = app.features.get::<Asked>(NAME).and_then(|asked| asked.0.take());
    let (true, Some(found)) = (mine, found) else { return false };
    app.ask = None;
    if domain::factory_reset::confirmed(decision) {
        app.ui.exit_chore = Some(Box::new(move || report(&wipe(&found))));
        app.ui.quit = true;
    } else {
        app.transcript.notice(format!("Nothing was erased; {} keeps everything.", brand::NAME));
    }
    true
}

fn platform_dirs() -> [Option<PathBuf>; 3] {
    [
        platform::data_dir(brand::NAME, brand::COMMAND),
        platform::local_data_dir(brand::NAME, brand::COMMAND),
        std::env::home_dir().map(|home| home.join(brand::DIR)),
    ]
}

fn targets(dirs: impl IntoIterator<Item = Option<PathBuf>>) -> Vec<PathBuf> {
    let mut targets: Vec<PathBuf> = Vec::new();
    for dir in dirs.into_iter().flatten() {
        if !targets.contains(&dir) {
            targets.push(dir);
        }
    }
    targets
}

fn existing(targets: Vec<PathBuf>) -> Vec<PathBuf> {
    targets.into_iter().filter(|dir| dir.exists()).collect()
}

fn wipe(found: &[PathBuf]) -> Vec<(PathBuf, std::io::Result<()>)> {
    found.iter().map(|dir| (dir.clone(), remove(dir))).collect()
}

const TRIES: u32 = 20;
const BETWEEN_TRIES: std::time::Duration = std::time::Duration::from_millis(100);

fn remove(dir: &Path) -> std::io::Result<()> {
    let mut tried = 1;
    loop {
        match std::fs::remove_dir_all(dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) if tried < TRIES => {
                tried += 1;
                std::thread::sleep(BETWEEN_TRIES);
            }
            done => return done,
        }
    }
}

fn report(outcomes: &[(PathBuf, std::io::Result<()>)]) -> String {
    let failed: Vec<String> = outcomes
        .iter()
        .filter_map(|(dir, outcome)| outcome.as_ref().err().map(|error| format!("  {}: {error}", dir.display())))
        .collect();
    match failed.is_empty() {
        true => format!("{} is reset: everything it kept on this machine is erased.", brand::NAME),
        false => format!("{} could not erase everything:\n{}", brand::NAME, failed.join("\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_targets_are_each_known_folder_once() {
        let data = PathBuf::from("/data");
        let found = targets([Some(data.clone()), Some(data.clone()), None, Some(PathBuf::from("/home/.app"))]);
        assert_eq!(found, [data, PathBuf::from("/home/.app")]);
    }

    #[test]
    fn a_reset_erases_the_folders_and_says_what_it_could_not() {
        let root = tempfile::tempdir().expect("dir");
        let (data, config) = (root.path().join("data"), root.path().join(".app"));
        std::fs::create_dir_all(data.join("sessions")).expect("sessions");
        std::fs::write(data.join("secrets.json"), "{}").expect("secrets");
        std::fs::create_dir_all(&config).expect("config");
        let found = existing(targets([Some(data.clone()), Some(config.clone()), Some(root.path().join("absent"))]));
        assert_eq!(found, [data.clone(), config.clone()], "only what exists is offered");
        let outcomes = wipe(&found);
        assert!(!data.exists() && !config.exists());
        assert!(report(&outcomes).contains("is reset"));
        let failed = [(data.clone(), Err(std::io::Error::other("busy")))];
        assert!(report(&failed).contains("busy"));
    }

    fn app(data: &Path) -> (App, agent_engine::fake::FakeEngine) {
        let engine = agent_engine::fake::FakeEngine::new(Vec::new(), Vec::new());
        let app = App::new(crate::tui::tests::client(&engine, data), data.into(), None, None);
        (app, engine)
    }

    #[test]
    fn the_reset_asks_and_erases_only_when_erase_is_picked() {
        let root = tempfile::tempdir().expect("dir");
        let kept = root.path().join("kept");
        std::fs::create_dir_all(&kept).expect("kept");
        let (mut app, _engine) = app(root.path());
        offer(&mut app, vec![kept.clone()]);
        assert!(app.ask.as_ref().is_some_and(|pending| pending.ask.id.0 == domain::factory_reset::ASK));
        app.answer(app::Decision::Deny);
        assert!(app.ask.is_none() && !app.ui.quit && app.ui.exit_chore.is_none());
        assert!(kept.exists(), "closing the question erases nothing");

        offer(&mut app, vec![kept.clone()]);
        app.answer(app::Decision::Answer(vec![vec![1]]));
        assert!(app.ui.quit, "erasing quits first");
        let chore = app.ui.exit_chore.take().expect("the erase waits for the exit");
        assert!(chore().contains("is reset"));
        assert!(!kept.exists());
    }

    fn agent_question() -> domain::AgentAsk {
        let choice = |label: &str| domain::Choice { label: label.into(), description: String::new() };
        let question = domain::Question {
            header: "Port".into(),
            text: "Which port?".into(),
            options: vec![choice("3000"), choice("8080")],
            multi: false,
        };
        domain::AgentAsk { id: domain::AskId("a-1".into()), kind: domain::AskKind::Questions(vec![question]) }
    }

    fn asking(data: &Path, script: Vec<app::EngineEvent>) -> (App, agent_engine::fake::FakeEngine) {
        let model = app::Model::new("keyed:scripted", "Scripted");
        let engine =
            agent_engine::fake::FakeEngine::new(vec![model], script).with_delay(std::time::Duration::from_millis(60));
        let mut app = App::new(crate::tui::tests::client(&engine, data), data.into(), None, None);
        crate::tui::tests::until(&mut app, |app| app.session.is_some());
        (app, engine)
    }

    fn second_option() -> app::Decision {
        app::Decision::Answer(vec![vec![1]])
    }

    #[test]
    fn an_agent_question_over_the_reset_question_is_answered_to_the_agent_and_erases_nothing() {
        let root = tempfile::tempdir().expect("dir");
        let kept = root.path().join("kept");
        std::fs::create_dir_all(&kept).expect("kept");
        let done = app::EngineEvent::Completed { duration_ms: 1, cost_usd: None };
        let (mut app, engine) = asking(root.path(), vec![app::EngineEvent::Asked(agent_question()), done]);
        offer(&mut app, vec![kept.clone()]);
        app.send("pick a port".into());
        crate::tui::tests::until(&mut app, |app| app.ask.as_ref().is_some_and(|pending| pending.ask.id.0 == "a-1"));

        app.answer(second_option());
        assert!(!app.ui.quit && app.ui.exit_chore.is_none(), "the answer was for the agent, not the reset");
        crate::tui::tests::until(&mut app, |_| !engine.answers().is_empty());
        assert_eq!(engine.answers(), [(domain::AskId("a-1".into()), second_option())]);
        assert!(kept.exists());
    }

    #[test]
    fn a_run_ending_keeps_the_reset_question_and_a_later_agent_answer_erases_nothing() {
        let root = tempfile::tempdir().expect("dir");
        let kept = root.path().join("kept");
        std::fs::create_dir_all(&kept).expect("kept");
        let (mut app, _engine) =
            asking(root.path(), vec![app::EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        offer(&mut app, vec![kept.clone()]);
        app.send("quick one".into());
        crate::tui::tests::until(&mut app, |app| !app.busy() && app.transcript.entries.len() > 1);
        assert!(
            app.ask.as_ref().is_some_and(|pending| pending.ask.id.0 == domain::factory_reset::ASK),
            "the run that ended never asked it, so it stays"
        );

        app.ask = Some(Pending::new(agent_question()));
        app.answer(second_option());
        assert!(!app.ui.quit && app.ui.exit_chore.is_none());
        assert!(kept.exists());
    }

    #[test]
    fn the_reset_is_refused_while_the_agent_works() {
        let root = tempfile::tempdir().expect("dir");
        let done = app::EngineEvent::Completed { duration_ms: 1, cost_usd: None };
        let (mut app, engine) = asking(root.path(), vec![app::EngineEvent::Asked(agent_question()), done]);
        app.send("pick a port".into());
        crate::tui::tests::until(&mut app, |app| app.ask.is_some());
        guarded(&mut app, vec![root.path().to_owned()]);
        assert!(app.ask.as_ref().is_some_and(|pending| pending.ask.id.0 == "a-1"), "the agent's question stays");
        let last = format!("{:?}", app.transcript.entries.last());
        assert!(last.contains("Wait for the agent to finish"), "{last}");
        app.answer(second_option());
        assert!(!app.ui.quit && app.ui.exit_chore.is_none());
        crate::tui::tests::until(&mut app, |_| !engine.answers().is_empty());
    }

    #[test]
    fn a_folder_still_being_written_is_erased_once_the_writer_stops() {
        let root = tempfile::tempdir().expect("dir");
        let data = root.path().join("data");
        let users = data.join("users");
        std::fs::create_dir_all(&users).expect("users");
        for at in 0..200 {
            std::fs::write(data.join(format!("session-{at}.jsonl")), "{}").expect("session");
        }
        let writer = std::thread::spawn(move || {
            let until = std::time::Instant::now() + std::time::Duration::from_millis(300);
            let mut at = 0;
            while std::time::Instant::now() < until {
                let _ = std::fs::write(users.join(format!("settings-{at}.json")), "{}");
                at += 1;
            }
        });
        let erased = remove(&data);
        writer.join().expect("writer");
        assert!(erased.is_ok(), "{erased:?}");
        assert!(!data.exists());
    }
}
