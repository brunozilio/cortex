use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use app::{Client, Model};
use app::{InMemoryProviderStore, InMemorySecretStore, LocalServices};
use domain::provider::ModelProvider;
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::tests::{press, screen, type_text, until};

const KEYED: ModelProvider = ModelProvider::new("keyed");

#[derive(Debug)]
struct NothingListed;

impl app::ModelCatalog for NothingListed {
    fn models(
        &self,
        _provider: ModelProvider,
        _key: domain::provider::ProviderKey,
    ) -> Result<Vec<domain::ModelOption>, String> {
        Ok(Vec::new())
    }
}

fn client(engine: &FakeEngine, data: &std::path::Path) -> Client {
    agent_engine::providers::install_testing();
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.homes = Arc::new(app::OwnerHome::at(data));
    services.providers = services
        .providers
        .with_secrets(Arc::new(InMemorySecretStore::default()))
        .with_store(Arc::new(InMemoryProviderStore::default()))
        .with_catalog(Arc::new(NothingListed));
    Client::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

fn two_accounts(data: &std::path::Path) -> App {
    let engine = FakeEngine::new(vec![Model::new("keyed:main", "Main").served_by(KEYED)], Vec::new());
    let client = client(&engine, data);
    client.connect_provider(KEYED, "sk-proj-0123456789aaaa").expect("first");
    client.connect_provider(KEYED, "sk-proj-0123456789bbbb").expect("second");
    let mut app = App::new(client, data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app
}

fn in_use(app: &App) -> Option<String> {
    let providers = app.client.providers().expect("providers");
    providers.into_iter().find(|p| p.provider == KEYED).and_then(|p| p.account).map(|a| a.label)
}

fn command(app: &mut App, text: &str) {
    type_text(app, text);
    press(app, KeyCode::Enter);
    until(app, |app| app.overlays.picker.is_some());
}

#[test]
fn accounts_lists_every_account_and_marks_the_one_in_use() {
    let data = tempfile::tempdir().expect("data");
    let mut app = two_accounts(data.path());
    command(&mut app, "/accounts");
    let shown = screen(&app);
    assert!(shown.contains("…aaaa") && shown.contains("…bbbb"), "{shown}");
    assert!(shown.contains("in use"), "{shown}");
}

#[test]
fn picking_an_account_makes_it_the_one_in_use() {
    let data = tempfile::tempdir().expect("data");
    let mut app = two_accounts(data.path());
    assert_eq!(in_use(&app).as_deref(), Some("…bbbb"));
    command(&mut app, "/accounts");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| in_use(app).as_deref() == Some("…aaaa"));
    until(&mut app, |app| screen(app).contains("Using …aaaa"));
}

#[test]
fn accounts_remove_forgets_the_picked_account() {
    let data = tempfile::tempdir().expect("data");
    let mut app = two_accounts(data.path());
    command(&mut app, "/accounts remove");
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| in_use(app).as_deref() == Some("…aaaa"));
    let left = app.client.providers().expect("providers").into_iter().find(|p| p.provider == KEYED).expect("keyed");
    assert_eq!(left.accounts.len(), 1);
}

#[test]
fn accounts_balance_sets_how_the_session_provider_s_accounts_share_the_work() {
    use domain::account_pool::Balance;
    let data = tempfile::tempdir().expect("data");
    let mut app = two_accounts(data.path());
    type_text(&mut app, "/accounts balance round-robin");
    press(&mut app, KeyCode::Enter);
    let balance = |app: &App| {
        let providers = app.client.providers().expect("providers");
        providers.into_iter().find(|p| p.provider == KEYED).map(|p| p.balance)
    };
    until(&mut app, |app| balance(app) == Some(Balance::RoundRobin));
    until(&mut app, |app| screen(app).contains("round-robin"));
}
