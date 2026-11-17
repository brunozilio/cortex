use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use app::{InMemorySecretStore, LocalServices, Model, Trust};

use super::*;

struct Project {
    client: Client,
    engine: FakeEngine,
    repo: PathBuf,
    data: PathBuf,
    home: PathBuf,
    _folders: tempfile::TempDir,
}

fn client_of(engine: &FakeEngine, data: &Path, home: &Path) -> Client {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.off.insert(domain::features::MEMORY_SEARCH.into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    services.homes = Arc::new(app::OwnerHome::at(home));
    Client::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

fn project_with_a_hook() -> Project {
    let folders = tempfile::tempdir().expect("folders");
    let at = |name: &str| folders.path().join(name);
    let (data, home, repo) = (at("data"), at("home"), at("rocket"));
    let hooks = repo.join(brand::DIR).join("hooks");
    for folder in [&data, &home, &hooks] {
        std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(hooks.join("guard.js"), "export const event = \"PreToolUse\";\nexport default () => ({});\n")
        .expect("hook");
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![Model::new("scripted", "Scripted")], done);
    let client = client_of(&engine, &data, &home);
    Project { client, engine, repo, data, home, _folders: folders }
}

fn said(client: &Client, cwd: &Path, trust: bool) -> String {
    let mut errors = Vec::new();
    settle_trust(client, cwd, trust, &mut errors).expect("settled");
    String::from_utf8(errors).expect("text")
}

fn trust_the_run_got(engine: &FakeEngine) -> Option<Trust> {
    engine.requests().last().and_then(|request| request.host.extensions.as_ref().map(|extensions| extensions.trust))
}

#[test]
fn printing_without_trust_skips_what_the_folder_brings_and_says_so_in_one_line() {
    let project = project_with_a_hook();

    let said_once = said(&project.client, &project.repo, false);

    assert!(said_once.starts_with(&format!("{}: this folder is not trusted", brand::COMMAND)), "{said_once}");
    assert!(said_once.contains("1 project item was skipped: Hook scripts (1)"), "{said_once}");
    assert!(said_once.contains("--trust"), "{said_once}");
    assert_eq!(said_once.lines().count(), 1, "{said_once}");
    assert!(said(&project.client, &project.data, false).is_empty(), "nothing to say where nothing was skipped");

    let code = print_all(&project.client, project.repo.clone(), Wanted::default(), [Ok("hi".into())]);
    assert_eq!(code, Ok(ExitCode::SUCCESS));
    assert_eq!(trust_the_run_got(&project.engine), Some(Trust::Untrusted), "and nobody was asked");
}

#[test]
fn printing_with_trust_runs_trusted_says_nothing_and_stores_nothing() {
    let project = project_with_a_hook();
    let wanted = Wanted { trust: true, ..Wanted::default() };

    let code = print_all(&project.client, project.repo.clone(), wanted, [Ok("hi".into())]);

    assert_eq!(code, Ok(ExitCode::SUCCESS));
    assert_eq!(trust_the_run_got(&project.engine), Some(Trust::Trusted));
    assert!(said(&project.client, &project.repo, true).is_empty(), "nothing was skipped");
    let another_run = client_of(&project.engine, &project.data, &project.home);
    let state = another_run.trust_state(&project.repo).expect("state");
    assert_eq!(state.trust, Trust::Untrusted, "the flag is for one invocation");
}

#[test]
fn trust_asked_for_in_the_home_folder_is_refused_aloud_and_the_run_goes_on_untrusted() {
    let project = project_with_a_hook();
    let wanted = Wanted { trust: true, ..Wanted::default() };

    let refused = said(&project.client, &project.home, true);
    assert!(refused.starts_with(&format!("{}: --trust ignored: ", brand::COMMAND)), "{refused}");
    assert!(refused.contains("home folder cannot be trusted"), "{refused}");
    assert_eq!(refused.lines().count(), 1, "{refused}");

    let code = print_all(&project.client, project.home.clone(), wanted, [Ok("hi".into())]);
    assert_eq!(code, Ok(ExitCode::SUCCESS), "the home folder has nothing of a project's to load");
    assert_eq!(trust_the_run_got(&project.engine), Some(Trust::Untrusted));
}

#[test]
fn the_replaced_setting_is_pointed_out_and_enables_nothing() {
    let project = project_with_a_hook();

    let settings = project.home.join(brand::DIR);
    std::fs::create_dir_all(&settings).expect("settings folder");
    std::fs::write(settings.join("settings.json"), r#"{"trustProjectHooks": true}"#).expect("settings");
    let said_twice = said(&project.client, &project.repo, false);
    assert_eq!(said_twice.lines().count(), 2, "the setting enables nothing, and says so: {said_twice}");
    assert!(said_twice.contains("Hook scripts (1)") && said_twice.contains("trustProjectHooks"), "{said_twice}");
}
