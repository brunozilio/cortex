use std::sync::Arc;

use domain::provider::{ModelProvider, ProviderSummary};

use super::*;
use crate::secrets::InMemorySecretStore;

struct Rig {
    providers: ModelProviders,
    secrets: Arc<InMemorySecretStore>,
}

fn rig() -> Rig {
    let secrets = Arc::new(InMemorySecretStore::default());
    let providers =
        ModelProviders::new(Arc::new(InMemoryProviderStore::default()), secrets.clone()).with_clock(|| 1_000);
    Rig { providers, secrets }
}

const OWNER: Principal = Principal::LocalOwner;

fn api_key(rig: &Rig, provider: ModelProvider) -> String {
    let key = rig.providers.key(&OWNER, provider).expect("key");
    Config::parse(provider, key.current().expect("load").secret()).expect("config").api_key
}

fn listed(event: &ServerEvent) -> Vec<ProviderSummary> {
    match event {
        ServerEvent::Providers(ProviderEvent::Listed { providers }) => providers.clone(),
        other => panic!("expected the list, got {other:?}"),
    }
}

fn failure(event: &ServerEvent) -> String {
    match event {
        ServerEvent::Providers(ProviderEvent::Failed { message, .. }) => message.clone(),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn an_empty_owner_lists_every_provider_with_an_account() {
    let list = listed(&rig().providers.list(&OWNER));
    assert_eq!(list.iter().map(|s| s.provider).collect::<Vec<_>>(), ModelProvider::ACCOUNTS);
    assert!(list.iter().all(|s| s.account.is_none()));
}

#[test]
fn a_key_is_kept_as_a_secret_and_listed_only_by_its_end() {
    let rig = rig();
    let event = rig.providers.connect(&OWNER, ModelProvider::OpenAi, "  sk-proj-0123456789abcd  ");
    let account = listed(&event)[0].account.clone().expect("connected");
    assert_eq!((account.label.as_str(), account.since_unix), ("…abcd", 1_000));
    let json = serde_json::to_string(&event).expect("json");
    assert!(!json.contains("sk-proj"), "the key never leaves: {json}");
    assert_eq!(api_key(&rig, ModelProvider::OpenAi), "sk-proj-0123456789abcd");
}

#[test]
fn a_blank_key_fails_and_keeps_nothing() {
    let rig = rig();
    assert_eq!(failure(&rig.providers.connect(&OWNER, ModelProvider::Anthropic, "   ")), "the key is empty");
    assert!(rig.secrets.get(&OWNER, secret_id(ModelProvider::Anthropic)).is_none());
    let missing = rig.providers.key(&OWNER, ModelProvider::Anthropic).expect_err("no key");
    assert!(missing.contains("not connected"), "{missing}");
}

#[test]
fn a_new_key_replaces_the_old_and_disconnecting_forgets_it() {
    let rig = rig();
    rig.providers.connect(&OWNER, ModelProvider::OpenAi, "sk-proj-0123456789abcd");
    rig.providers.connect(&OWNER, ModelProvider::OpenAi, "sk-proj-0123456789wxyz");
    assert_eq!(api_key(&rig, ModelProvider::OpenAi), "sk-proj-0123456789wxyz");
    assert!(listed(&rig.providers.disconnect(&OWNER, ModelProvider::OpenAi))[0].account.is_none());
    assert!(rig.secrets.get(&OWNER, secret_id(ModelProvider::OpenAi)).is_none());
}

#[test]
fn owners_keep_their_providers_apart() {
    let rig = rig();
    rig.providers.connect(&OWNER, ModelProvider::OpenAi, "sk-proj-0123456789abcd");
    let member = Principal::member(domain::UserId::new(), domain::OrgId::new());
    assert!(listed(&rig.providers.list(&member))[0].account.is_none());
    assert!(rig.providers.key(&member, ModelProvider::OpenAi).is_err());
}

#[test]
fn subscriptions_accept_upstream_token_json_and_never_list_tokens() {
    let rig = rig();
    let raw = r#"{"access_token":"access-secret","refresh_token":"refresh-secret","account_id":"acct","expires_at":"2099-01-01T00:00:00Z"}"#;
    for provider in [ModelProvider::ChatGptSubscription, ModelProvider::ClaudeSubscription] {
        let event = rig.providers.connect(&OWNER, provider, raw);
        assert!(listed(&event).iter().any(|p| p.provider == provider && p.account.is_some()));
        assert!(!serde_json::to_string(&event).expect("event").contains("secret"));
        assert!(rig.providers.key(&OWNER, provider).expect("key").current().is_ok());
        assert!(matches!(
            rig.providers.connect(&OWNER, provider, "sk-api-key"),
            ServerEvent::Providers(ProviderEvent::Failed { .. })
        ));
    }
}

struct FakeSignIn(Result<(), String>);

impl SignIn for FakeSignIn {
    fn sign_in(&self, provider: ModelProvider, open: &mut dyn FnMut(String, Option<String>)) -> Result<Config, String> {
        let code = (provider == ModelProvider::CopilotSubscription).then(|| "AB12-CD34".to_owned());
        open(format!("https://auth.test/{}", provider.id()), code);
        self.0.clone()?;
        let raw = r#"{"access_token":"access-secret","refresh_token":"refresh-secret","account_id":"acct","expires_at":"2099-01-01T00:00:00Z"}"#;
        Config::parse(provider, raw).map_err(|e| e.to_string())
    }
}

fn outbox() -> (Outbox, crossbeam_channel::Receiver<ServerEvent>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    (Outbox::new(tx, Arc::new(|| {})), rx)
}

fn next(rx: &crossbeam_channel::Receiver<ServerEvent>) -> ServerEvent {
    rx.recv_timeout(std::time::Duration::from_secs(5)).expect("event")
}

#[test]
fn signing_in_to_a_subscription_opens_the_browser_then_keeps_the_tokens() {
    let rig = rig();
    let providers = rig.providers.clone().with_sign_in(Arc::new(FakeSignIn(Ok(()))));
    let (outbox, rx) = outbox();
    providers.sign_in(&OWNER, ModelProvider::ClaudeSubscription, &outbox);
    assert!(matches!(
        next(&rx),
        ServerEvent::Providers(ProviderEvent::AuthorizeAt { provider: ModelProvider::ClaudeSubscription, url, code: None })
            if url.ends_with("claude-subscription")
    ));
    let listed = listed(&next(&rx));
    assert!(listed.iter().any(|p| p.provider == ModelProvider::ClaudeSubscription && p.account.is_some()));
    let key = providers.key(&OWNER, ModelProvider::ClaudeSubscription).expect("key");
    assert!(key.current().expect("load").secret().contains("access-secret"));
}

#[test]
fn copilot_signs_in_with_a_code_to_type_and_keeps_its_own_account() {
    let rig = rig();
    let providers = rig.providers.clone().with_sign_in(Arc::new(FakeSignIn(Ok(()))));
    let (outbox, rx) = outbox();
    providers.sign_in(&OWNER, ModelProvider::CopilotSubscription, &outbox);
    assert!(matches!(
        next(&rx),
        ServerEvent::Providers(ProviderEvent::AuthorizeAt { provider: ModelProvider::CopilotSubscription, code: Some(code), .. })
            if code == "AB12-CD34"
    ));
    let listed = listed(&next(&rx));
    let connected: Vec<_> = listed.iter().filter(|p| p.account.is_some()).map(|p| p.provider).collect();
    assert_eq!(connected, [ModelProvider::CopilotSubscription]);
    assert!(providers.key(&OWNER, ModelProvider::ClaudeSubscription).is_err());
}

#[test]
fn a_failed_sign_in_or_an_api_provider_keeps_nothing() {
    let rig = rig();
    let providers = rig.providers.clone().with_sign_in(Arc::new(FakeSignIn(Err("closed the tab".into()))));
    let (outbox, rx) = outbox();
    providers.sign_in(&OWNER, ModelProvider::ChatGptSubscription, &outbox);
    next(&rx);
    assert_eq!(failure(&next(&rx)), "closed the tab");
    providers.sign_in(&OWNER, ModelProvider::Anthropic, &outbox);
    assert!(failure(&next(&rx)).contains("API key"));
    assert!(listed(&providers.list(&OWNER)).iter().all(|p| p.account.is_none()));
}

struct ByKey;

impl ModelCatalog for ByKey {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<domain::ModelOption>, String> {
        let secret = key.current()?.secret().to_owned();
        let api_key = Config::parse(provider, &secret).map_err(|e| e.to_string())?.api_key;
        match api_key.strip_suffix("-0123456789") {
            Some("sk-broken") => Err("401".into()),
            Some(name) => Ok(vec![domain::ModelOption::new(format!("{}:{name}", provider.id()), name)]),
            None => Err("no key".into()),
        }
    }
}

fn discovered(providers: &ModelProviders, local: Vec<domain::ModelOption>) -> (Vec<String>, Vec<ServerEvent>) {
    let (outbox, rx) = outbox();
    providers.discover_models(&OWNER, local, &outbox);
    let mut others = Vec::new();
    loop {
        match next(&rx) {
            ServerEvent::Models(models) => return (models.into_iter().map(|m| m.id).collect(), others),
            other => others.push(other),
        }
    }
}

#[test]
fn each_connected_provider_lists_its_models_and_the_engine_s_come_last() {
    let providers = rig().providers.with_catalog(Arc::new(ByKey));
    assert!(discovered(&providers, Vec::new()).0.is_empty(), "no fixed list without an account");
    providers.connect(&OWNER, ModelProvider::Anthropic, "sk-ant-0123456789");
    providers.connect(&OWNER, ModelProvider::OpenAi, "sk-broken-0123456789");
    let fixture = domain::ModelOption::new("fixture", "Fixture").served_by(ModelProvider::OpenAi);
    let (models, events) = discovered(&providers, vec![fixture]);
    assert_eq!(models, ["anthropic:sk-ant", "fixture"]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            ServerEvent::Providers(ProviderEvent::Failed { provider: ModelProvider::OpenAi, .. })
        ))
    );
}
