use std::path::Path;
use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use app::{Client, Decision, EngineEvent, InMemorySecretStore, LocalServices, Trust};
use ratatui::crossterm::event::KeyCode;

use super::*;
use crate::tui::event_loop::Opening;
use crate::tui::tests::{press, screen, until};

struct Stage {
    app: App,
    engine: FakeEngine,
    repo: std::path::PathBuf,
    _folders: tempfile::TempDir,
}

fn stage(hook: bool) -> Stage {
    let folders = tempfile::tempdir().expect("folders");
    let (home, repo, data) = (folders.path().join("home"), folders.path().join("rocket"), folders.path().join("data"));
    for folder in [&home, &repo, &data] {
        std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(repo.join(brand::INSTRUCTIONS), "Use tabs.").expect("instructions");
    if hook {
        let hooks = repo.join(brand::DIR).join("hooks");
        std::fs::create_dir_all(&hooks).expect("hooks");
        std::fs::write(hooks.join("guard.js"), "export const event = \"Stop\"; export default () => ({});")
            .expect("hook");
    }
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![app::Model::new("keyed:scripted", "Scripted")], done);
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.homes = Arc::new(app::OwnerHome::at(&home));
    services.off.extend([domain::features::MEMORY_SEARCH, domain::features::GIT_OFFER].map(str::to_owned));
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    let client = Client::builder().data_dir(&data).services(services).build().expect(brand::COMMAND);
    let mut app = App::new(client, repo.clone(), None, None);
    opening(&mut app);
    until(&mut app, |app| app.session.is_some());
    Stage { app, engine, repo, _folders: folders }
}

fn trust_of(app: &App, folder: &Path) -> Trust {
    app.client.trust_state(folder).expect("state").trust
}

fn last_notice(app: &App) -> String {
    format!("{:?}", app.transcript.entries.last())
}

#[test]
fn a_folder_that_brings_something_asks_before_the_first_prompt_and_continuing_is_the_default() {
    let Stage { mut app, engine, repo, _folders } = stage(true);
    let mut opening = Opening { prompt: Some("fix the build".into()), pick: false };

    opening.step(&mut app);
    assert!(pending(&app), "the dialog is up");
    assert!(engine.requests().is_empty(), "nothing was sent while it waits");
    let shown = screen(&app);
    for said in ["Do you trust the files in", "hooks/guard.js", "Trust this folder", "❯ 2. Continue without", "Exit"]
    {
        assert!(shown.contains(said), "{said} in:\n{shown}");
    }

    press(&mut app, KeyCode::Enter);
    assert!(!pending(&app));
    assert_eq!(trust_of(&app, &repo), Trust::Untrusted, "Enter took the default");
    assert!(last_notice(&app).contains("Continuing without trusting"), "{}", last_notice(&app));
    opening.step(&mut app);
    until(&mut app, |_| !engine.requests().is_empty());
    assert_eq!(engine.requests()[0].prompt.text, "fix the build");
    let carried = engine.requests()[0].host.extensions.as_ref().map(|extensions| extensions.trust);
    assert_eq!(carried, Some(Trust::Untrusted));
}

#[test]
fn trusting_in_the_dialog_is_stored_and_the_first_prompt_runs_trusted() {
    let Stage { mut app, engine, repo, _folders } = stage(true);
    let mut opening = Opening { prompt: Some("fix the build".into()), pick: false };

    app.answer(Decision::Answer(vec![vec![0]]));

    assert_eq!(trust_of(&app, &repo), Trust::Trusted);
    assert!(last_notice(&app).contains("is now trusted"), "{}", last_notice(&app));
    opening.step(&mut app);
    until(&mut app, |_| !engine.requests().is_empty());
    let carried = engine.requests()[0].host.extensions.as_ref().map(|extensions| extensions.trust);
    assert_eq!(carried, Some(Trust::Trusted));

    let mut again = App::new(app.client.clone(), repo, None, None);
    super::opening(&mut again);
    assert!(!pending(&again), "a trusted folder does not ask again");
}

#[test]
fn exit_leaves_without_sending_or_trusting_and_closing_the_dialog_continues() {
    let Stage { mut app, engine, repo, _folders } = stage(true);
    app.answer(Decision::Answer(vec![vec![2]]));
    assert!(app.ui.quit);
    assert!(engine.requests().is_empty());
    assert_eq!(trust_of(&app, &repo), Trust::Untrusted);

    let Stage { mut app, repo, _folders, .. } = stage(true);
    press(&mut app, KeyCode::Esc);
    assert!(!pending(&app) && !app.ui.quit);
    assert_eq!(trust_of(&app, &repo), Trust::Untrusted);
}

#[test]
fn a_folder_with_nothing_to_enable_does_not_ask() {
    let Stage { mut app, engine, _folders, .. } = stage(false);
    let mut opening = Opening { prompt: Some("hi".into()), pick: false };

    assert!(!pending(&app) && app.ask.is_none(), "an instruction file alone is nothing to agree to");
    opening.step(&mut app);
    until(&mut app, |_| !engine.requests().is_empty());
}

#[test]
fn the_trust_command_shows_the_state_and_trusts_and_untrusts_for_the_next_run() {
    let Stage { mut app, repo, _folders, .. } = stage(true);
    app.answer(Decision::Deny);
    let ran = |app: &mut App, argument: Option<&str>| {
        let before = app.transcript.entries.len();
        run(app, argument.map(str::to_owned));
        until(app, |app| app.transcript.entries.len() > before);
        last_notice(app)
    };

    let shown = ran(&mut app, None);
    for said in ["is not trusted", "Hook scripts", "hooks/guard.js", "/trust yes trusts it", "next run"] {
        assert!(shown.contains(said), "{said} in {shown}");
    }

    let trusted = ran(&mut app, Some("yes"));
    assert!(trusted.contains("is now trusted") && trusted.contains("next run"), "{trusted}");
    assert_eq!(trust_of(&app, &repo), Trust::Trusted);
    assert!(ran(&mut app, None).contains("/trust no stops trusting it"));

    let untrusted = ran(&mut app, Some("NO"));
    assert!(untrusted.contains("no longer trusted") && untrusted.contains("next run"), "{untrusted}");
    assert_eq!(trust_of(&app, &repo), Trust::Untrusted);

    assert!(ran(&mut app, Some("maybe")).contains("/trust yes trusts it"), "anything else says how to use it");
}
