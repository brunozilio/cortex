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
    app.ask = Some(Pending::new(domain::factory_reset::ask(brand::NAME, &folders)));
}

fn answered(app: &mut App, decision: &sdk::Decision) -> bool {
    let Some(found) = app.features.get::<Asked>(NAME).and_then(|asked| asked.0.take()) else { return false };
    app.ask = None;
    if domain::factory_reset::confirmed(decision) {
        app.exit_chore = Some(Box::new(move || report(&wipe(&found))));
        app.quit = true;
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
        let app = App::new(crate::tui::tests::sdk(&engine, data), data.into(), None, None);
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
        app.answer(sdk::Decision::Deny);
        assert!(app.ask.is_none() && !app.quit && app.exit_chore.is_none());
        assert!(kept.exists(), "closing the question erases nothing");

        offer(&mut app, vec![kept.clone()]);
        app.answer(sdk::Decision::Answer(vec![vec![1]]));
        assert!(app.quit, "erasing quits first");
        let chore = app.exit_chore.take().expect("the erase waits for the exit");
        assert!(chore().contains("is reset"));
        assert!(!kept.exists());
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
