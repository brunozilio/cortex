use super::*;
use crate::plugins::{InMemoryPluginStore, InMemorySecretStore, McpOAuth, SystemTokenCommand};
use superfik_core::PluginId;
use superfik_protocol::ServerEvent;

#[test]
fn catalog_accounts_are_isolated_and_live_credentials_stop_working_after_disconnect() {
    let dir = tempfile::tempdir().expect("dir");
    let store = Arc::new(InMemorySecretStore::default());
    let plugins = Plugins::new(
        Arc::new(InMemoryPluginStore::new(dir.path().to_path_buf())),
        store.clone(),
        Arc::new(McpOAuth::default()),
        Arc::new(SystemTokenCommand),
    );
    let owner = Principal::LocalOwner;
    let other = Principal::member(superfik_core::UserId::new(), superfik_core::OrgId::new());
    let (tx, rx) = crossbeam_channel::unbounded();
    let outbox = crate::outbox::Outbox::new(tx, Arc::new(|| {}));
    for provider in [ModelProvider::OpenAi, ModelProvider::Anthropic] {
        let id = PluginId(provider.plugin_id().into());
        assert!(matches!(plugins.install(&owner, Some(&id), None), ServerEvent::Plugins { .. }));
        let mut values = vec![("api_key".into(), "sk-test-secret".into())];
        if provider == ModelProvider::OpenAi {
            values.push(("base_url".into(), "https://api.openai.com/v1".into()));
        }
        plugins.connect(&owner, &id, "", &values, &outbox);
        let event = rx.recv().expect("event");
        assert!(!serde_json::to_string(&event).expect("json").contains("sk-test-secret"));
        let key = plugins.model_key(&owner, provider).expect("key");
        assert_eq!(
            Config::parse(provider, key.current().expect("load").secret()).expect("config").api_key,
            "sk-test-secret"
        );
        assert!(plugins.model_key(&other, provider).is_none());
        let ServerEvent::Plugins { connections, .. } = event else { panic!("not connected") };
        let connection = connections.iter().find(|c| c.plugin == id).expect("connection");
        plugins.disconnect(&owner, connection.id);
        assert!(key.current().is_err(), "an in-flight run cannot reuse a disconnected account");
        assert!(plugins.model_key(&owner, provider).is_none());
    }
}

#[test]
fn multiple_ai_accounts_keep_separate_secrets_and_a_run_uses_its_selection() {
    let dir = tempfile::tempdir().expect("dir");
    let plugins = Plugins::new(
        Arc::new(InMemoryPluginStore::new(dir.path().into())),
        Arc::new(InMemorySecretStore::default()),
        Arc::new(McpOAuth::default()),
        Arc::new(SystemTokenCommand),
    );
    let owner = Principal::LocalOwner;
    for provider in ModelProvider::ACCOUNTS {
        let id = PluginId(provider.plugin_id().into());
        plugins.install(&owner, Some(&id), None);
        let package = plugins.catalog_package(&id).expect("package");
        assert_eq!(package.manifest.accounts, superfik_core::Accounts::Many);
        for marker in ["first", "second"] {
            let config = if provider.subscription() {
                serde_json::json!({"auth":{"access_token":format!("token-{marker}"),"refresh_token":format!("refresh-{marker}"),"expires_at":"2099-01-01T00:00:00Z","account_id":marker}})
            } else {
                serde_json::json!({"api_key":format!("sk-{marker}")})
            };
            plugins
                .keep(&owner, package, marker.into(), &Secrets::from([("config".into(), config.to_string())]))
                .expect("keep");
        }
        let book = plugins.store.load(&owner);
        let accounts: Vec<_> = book.connections.iter().filter(|c| c.plugin == id).cloned().collect();
        assert_eq!(accounts.len(), 2, "connecting another account preserves the first");
        let first = plugins.model_key_for(&owner, provider, &accounts[..1]).expect("first").expect("key");
        let second = plugins.model_key_for(&owner, provider, &accounts[1..]).expect("second").expect("key");
        assert!(first.current().expect("load").secret().contains("first"));
        assert!(second.current().expect("load").secret().contains("second"));
        let stranger = Principal::member(superfik_core::UserId::new(), superfik_core::OrgId::new());
        assert!(plugins.model_key_for(&stranger, provider, &accounts[1..]).is_err());
        plugins.disconnect(&owner, accounts[1].id);
        assert!(
            plugins.model_key_for(&owner, provider, &accounts[1..]).is_err(),
            "never fall back to a different account"
        );
        assert!(second.current().is_err());
        assert!(first.current().is_ok(), "disconnect only the selected account");
    }
}

#[test]
fn rotating_subscription_credentials_survive_loading_a_new_key() {
    let secrets = Arc::new(InMemorySecretStore::default());
    let connection = ConnectionId::new();
    let original = r#"{"auth":{"access_token":"access-secret","refresh_token":"refresh-secret","expires_at":"2099-01-01T00:00:00Z","account_id":"acct"}}"#;
    secrets
        .set(&Principal::LocalOwner, connection, &Secrets::from([("config".into(), original.into())]))
        .expect("save");
    let source = AccountStore {
        secrets,
        owner: Principal::LocalOwner,
        connection,
        provider: ModelProvider::ChatGptSubscription,
    };
    let updated = original.replace("refresh-secret", "refresh-rotated");
    source.save(&updated).expect("rotate");
    assert!(source.load().expect("load").contains("refresh-rotated"));
}

struct ByKey;

impl ModelCatalog for ByKey {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<superfik_core::ModelOption>, String> {
        let secret = key.current().map_err(|e| e.to_string())?.secret().to_owned();
        let api_key = Config::parse(provider, &secret).map_err(|e| e.to_string())?.api_key;
        match api_key.strip_prefix("sk-") {
            Some("broken") => Err("401".into()),
            Some(name) => Ok(vec![superfik_core::ModelOption::new(
                superfik_core::EngineKind::Native,
                format!("{}:{name}-model", provider.plugin_id()),
                format!("{name} model"),
            )]),
            None => Err("no key".into()),
        }
    }
}

fn with_accounts(accounts: &[(ModelProvider, &str)]) -> (Plugins, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("dir");
    let plugins = Plugins::new(
        Arc::new(InMemoryPluginStore::new(dir.path().into())),
        Arc::new(InMemorySecretStore::default()),
        Arc::new(McpOAuth::default()),
        Arc::new(SystemTokenCommand),
    )
    .with_model_catalog(Arc::new(ByKey));
    for (provider, name) in accounts {
        let id = PluginId(provider.plugin_id().into());
        plugins.install(&Principal::LocalOwner, Some(&id), None);
        let package = plugins.catalog_package(&id).expect("package");
        let config = serde_json::json!({"api_key": format!("sk-{name}")}).to_string();
        plugins
            .keep(&Principal::LocalOwner, package, (*name).into(), &Secrets::from([("config".into(), config)]))
            .expect("keep");
    }
    (plugins, dir)
}

fn listed(
    plugins: &Plugins,
    local: Vec<superfik_core::ModelOption>,
) -> (Vec<superfik_core::ModelOption>, Vec<ServerEvent>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let outbox = crate::outbox::Outbox::new(tx, Arc::new(|| {}));
    let providers = crate::providers::ModelProviders::new(
        Arc::new(crate::providers::InMemoryProviderStore::default()),
        Arc::new(InMemorySecretStore::default()),
    );
    plugins.discover_models(&Principal::LocalOwner, &providers, local, Vec::new(), &outbox);
    let mut others = Vec::new();
    loop {
        match rx.recv_timeout(std::time::Duration::from_secs(5)).expect("models") {
            ServerEvent::Models(models) => return (models, others),
            other => others.push(other),
        }
    }
}

#[test]
fn each_account_lists_its_own_models_tagged_with_it_and_local_ones_last() {
    let (plugins, _dir) = with_accounts(&[
        (ModelProvider::Anthropic, "work"),
        (ModelProvider::Anthropic, "home"),
        (ModelProvider::OpenAi, "lab"),
    ]);
    let qwen = superfik_core::ModelOption::new(superfik_core::EngineKind::Native, "mlx/qwen", "Qwen")
        .served_by(ModelProvider::Local);
    let (models, _) = listed(&plugins, vec![qwen.clone()]);
    let book = plugins.store.load(&Principal::LocalOwner);
    let account = |label: &str| book.connections.iter().find(|c| c.label == label).map(|c| c.id);
    let rows: Vec<_> = models.iter().map(|m| (m.id.as_str(), m.provider, m.account)).collect();
    assert_eq!(
        rows,
        vec![
            ("openai:lab-model", ModelProvider::OpenAi, account("lab")),
            ("anthropic:work-model", ModelProvider::Anthropic, account("work")),
            ("anthropic:home-model", ModelProvider::Anthropic, account("home")),
            ("mlx/qwen", ModelProvider::Local, None),
        ]
    );
}

#[test]
fn nothing_connected_lists_no_cloud_model_and_a_failing_account_says_why() {
    let (plugins, _dir) = with_accounts(&[]);
    assert!(listed(&plugins, Vec::new()).0.is_empty(), "no fixed list without an account");
    let (plugins, _dir) = with_accounts(&[(ModelProvider::Anthropic, "broken"), (ModelProvider::OpenAi, "lab")]);
    let (models, events) = listed(&plugins, Vec::new());
    assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["openai:lab-model"]);
    assert!(events.iter().any(|e| matches!(
        e,
        ServerEvent::Providers(superfik_protocol::ProviderEvent::Failed { provider: ModelProvider::Anthropic, .. })
    )));
}

#[test]
fn a_run_uses_the_account_its_model_was_listed_by() {
    let (plugins, _dir) = with_accounts(&[(ModelProvider::Anthropic, "work"), (ModelProvider::Anthropic, "home")]);
    let book = plugins.store.load(&Principal::LocalOwner);
    let home = book.connections.iter().find(|c| c.label == "home").expect("home").clone();
    let model =
        superfik_core::ModelOption::new(superfik_core::EngineKind::Native, "anthropic:x", "x").of_account(home.id);
    let key = plugins.run_key(&Principal::LocalOwner, &model, &[]).expect("key").expect("some");
    assert!(key.current().expect("load").secret().contains("sk-home"));
    plugins.disconnect(&Principal::LocalOwner, home.id);
    assert!(plugins.run_key(&Principal::LocalOwner, &model, &[]).is_err(), "never another account's key");
}
