use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use domain::provider::{ModelProvider, ProviderSummary};

use super::*;
use crate::api::{RequestId, Unsolicited};
use crate::handler::outbox::{Outbox, Replies};
use crate::secrets::InMemorySecretStore;

struct Rig {
    providers: ModelProviders,
    secrets: Arc<InMemorySecretStore>,
}

fn rig() -> Rig {
    llm_providers::testing::install();
    let secrets = Arc::new(InMemorySecretStore::default());
    let providers =
        ModelProviders::new(Arc::new(InMemoryProviderStore::default()), secrets.clone()).with_clock(|| 1_000);
    Rig { providers, secrets }
}

fn api_key(rig: &Rig, provider: ModelProvider) -> String {
    let key = rig.providers.key(provider).expect("key");
    Config::parse(agent_engine::providers::get(provider).expect("known"), key.current().expect("load").secret())
        .expect("config")
        .api_key
}

fn listed(event: &Reply) -> Vec<ProviderSummary> {
    match event {
        Reply::Providers(ProviderEvent::Listed { providers }) => providers.clone(),
        other => panic!("expected the list, got {other:?}"),
    }
}

fn account(event: &Reply, provider: &str) -> Option<domain::provider::ProviderAccount> {
    listed(event).into_iter().find(|s| s.provider.id() == provider).and_then(|s| s.account)
}

fn failure(event: &Reply) -> String {
    match event {
        Reply::Providers(ProviderEvent::Failed { message, .. }) => message.clone(),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn an_empty_owner_lists_every_provider_with_an_account() {
    let list = listed(&rig().providers.list());
    assert_eq!(
        list.iter().map(|s| s.provider).collect::<Vec<_>>(),
        agent_engine::providers::all().into_iter().map(llm_providers::id).collect::<Vec<_>>()
    );
    assert!(list.iter().all(|s| !s.name.is_empty()));
    assert!(list.iter().all(|s| s.account.is_none()));
}

#[test]
fn a_key_is_kept_as_a_secret_and_listed_only_by_its_end() {
    let rig = rig();
    let event = rig.providers.connect(ModelProvider::new("keyed"), "  sk-proj-0123456789abcd  ");
    let account = account(&event, "keyed").expect("connected");
    assert_eq!((account.label.as_str(), account.since_unix), ("…abcd", 1_000));
    let json = serde_json::to_string(&event).expect("json");
    assert!(!json.contains("sk-proj"), "the key never leaves: {json}");
    assert_eq!(api_key(&rig, ModelProvider::new("keyed")), "sk-proj-0123456789abcd");
}

#[test]
fn a_blank_key_fails_and_keeps_nothing() {
    let rig = rig();
    assert_eq!(failure(&rig.providers.connect(ModelProvider::new("keyed-too"), "   ")), "the key is empty");
    assert!(rig.secrets.get(secret_id(ModelProvider::new("keyed-too"))).is_none());
    let missing = rig.providers.key(ModelProvider::new("keyed-too")).expect_err("no key");
    assert!(missing.contains("not connected"), "{missing}");
}

#[test]
fn a_new_key_replaces_the_old_and_disconnecting_forgets_it() {
    let rig = rig();
    rig.providers.connect(ModelProvider::new("keyed"), "sk-proj-0123456789abcd");
    rig.providers.connect(ModelProvider::new("keyed"), "sk-proj-0123456789wxyz");
    assert_eq!(api_key(&rig, ModelProvider::new("keyed")), "sk-proj-0123456789wxyz");
    assert!(account(&rig.providers.disconnect(ModelProvider::new("keyed")), "keyed").is_none());
    assert!(rig.secrets.get(secret_id(ModelProvider::new("keyed"))).is_none());
}

#[test]
fn subscriptions_accept_upstream_token_json_and_never_list_tokens() {
    let rig = rig();
    let raw = r#"{"access_token":"access-secret","refresh_token":"refresh-secret","account_id":"acct","expires_at":"2099-01-01T00:00:00Z"}"#;
    for provider in [ModelProvider::new("browser-sub"), ModelProvider::new("device-sub")] {
        let event = rig.providers.connect(provider, raw);
        assert!(listed(&event).iter().any(|p| p.provider == provider && p.account.is_some()));
        assert!(!serde_json::to_string(&event).expect("event").contains("secret"));
        assert!(rig.providers.key(provider).expect("key").current().is_ok());
        assert!(matches!(
            rig.providers.connect(provider, "sk-api-key"),
            Reply::Providers(ProviderEvent::Failed { .. })
        ));
    }
}

struct FakeSignIn(Result<(), llm_providers::ProviderError>);

impl SignIn for FakeSignIn {
    fn sign_in(
        &self,
        provider: ModelProvider,
        _stop: &Arc<AtomicBool>,
        open: &mut dyn FnMut(String, Option<String>),
    ) -> Result<Config, llm_providers::ProviderError> {
        let code = (provider == ModelProvider::new("device-sub")).then(|| "AB12-CD34".to_owned());
        open(format!("https://auth.test/{}", provider.id()), code);
        self.0.clone()?;
        let raw = r#"{"access_token":"access-secret","refresh_token":"refresh-secret","account_id":"acct","expires_at":"2099-01-01T00:00:00Z"}"#;
        Config::parse(agent_engine::providers::get(provider).expect("known"), raw)
    }
}

type Sent = crossbeam_channel::Receiver<crate::api::ServerEvent>;

fn outbox() -> (Replies, Sent) {
    let (tx, rx) = crossbeam_channel::unbounded();
    (Outbox::new(tx, Arc::new(|| {})).answering(RequestId(1)), rx)
}

fn sent(rx: &Sent) -> crate::api::ServerEvent {
    rx.recv_timeout(std::time::Duration::from_secs(5)).expect("event")
}

fn next(rx: &Sent) -> Reply {
    match sent(rx) {
        crate::api::ServerEvent::Reply { id: RequestId(1), reply } => reply,
        other => panic!("expected an answer to the request, got {other:?}"),
    }
}

#[test]
fn signing_in_to_a_subscription_opens_the_browser_then_keeps_the_tokens() {
    let rig = rig();
    let providers = rig.providers.clone().with_sign_in(Arc::new(FakeSignIn(Ok(()))));
    let (outbox, rx) = outbox();
    providers.sign_in(ModelProvider::new("browser-sub"), &outbox);
    assert!(matches!(
        next(&rx),
        Reply::Providers(ProviderEvent::AuthorizeAt { provider, url, code: None })
            if provider == ModelProvider::new("browser-sub") && url.ends_with("browser-sub")
    ));
    let listed = listed(&next(&rx));
    assert!(listed.iter().any(|p| p.provider == ModelProvider::new("browser-sub") && p.account.is_some()));
    let key = providers.key(ModelProvider::new("browser-sub")).expect("key");
    assert!(key.current().expect("load").secret().contains("access-secret"));
}

#[test]
fn a_code_sign_in_shows_the_code_to_type_and_keeps_its_own_account() {
    let rig = rig();
    let providers = rig.providers.clone().with_sign_in(Arc::new(FakeSignIn(Ok(()))));
    let (outbox, rx) = outbox();
    providers.sign_in(ModelProvider::new("device-sub"), &outbox);
    assert!(matches!(
        next(&rx),
        Reply::Providers(ProviderEvent::AuthorizeAt { provider, code: Some(code), .. })
            if provider == ModelProvider::new("device-sub") && code == "AB12-CD34"
    ));
    let listed = listed(&next(&rx));
    let connected: Vec<_> = listed.iter().filter(|p| p.account.is_some()).map(|p| p.provider).collect();
    assert_eq!(connected, [ModelProvider::new("device-sub")]);
    assert!(providers.key(ModelProvider::new("browser-sub")).is_err());
}

#[test]
fn a_failed_sign_in_or_an_api_provider_keeps_nothing() {
    let rig = rig();
    let providers = rig
        .providers
        .clone()
        .with_sign_in(Arc::new(FakeSignIn(Err(llm_providers::ProviderError::Invalid("closed the tab".into())))));
    let (outbox, rx) = outbox();
    providers.sign_in(ModelProvider::new("browser-sub"), &outbox);
    next(&rx);
    assert_eq!(failure(&next(&rx)), "closed the tab");
    providers.sign_in(ModelProvider::new("keyed-too"), &outbox);
    assert!(failure(&next(&rx)).contains("API key"));
    assert!(listed(&providers.list()).iter().all(|p| p.account.is_none()));
}

fn unkept() -> ModelProviders {
    llm_providers::testing::install();
    let secrets = Arc::new(crate::secrets::Unkept::because("the disk is read-only"));
    ModelProviders::new(Arc::new(InMemoryProviderStore::default()), secrets).with_sign_in(Arc::new(FakeSignIn(Ok(()))))
}

#[test]
fn connecting_fails_clearly_when_keys_cannot_be_kept() {
    let providers = unkept();
    let refused = "keys cannot be kept on this machine: the disk is read-only";
    assert_eq!(failure(&providers.connect(ModelProvider::new("keyed"), "sk-proj-0123456789abcd")), refused);
    assert!(listed(&providers.list()).iter().all(|p| p.account.is_none()));
    let (outbox, rx) = outbox();
    providers.sign_in(ModelProvider::new("browser-sub"), &outbox);
    assert_eq!(failure(&next(&rx)), refused, "the browser is not opened for a sign-in that would be lost");
}

#[test]
fn the_client_is_told_once_that_keys_cannot_be_kept() {
    let providers = unkept();
    let notice = providers.clone().unkept_notice().expect("told");
    assert!(notice.starts_with("keys cannot be kept on this machine: the disk is read-only"), "{notice}");
    assert_eq!(providers.unkept_notice(), None, "once for the whole process");
    assert_eq!(rig().providers.unkept_notice(), None, "nothing to say when keys are kept");
}

struct ByKey;

impl ModelCatalog for ByKey {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<domain::ModelOption>, String> {
        let secret = key.current()?.secret().to_owned();
        let api_key = Config::parse(agent_engine::providers::get(provider).expect("known"), &secret)
            .map_err(|e| e.to_string())?
            .api_key;
        match api_key.strip_suffix("-0123456789") {
            Some("sk-broken") => Err("401".into()),
            Some(name) => Ok(vec![domain::ModelOption::new(format!("{}:{name}", provider.id()), name)]),
            None => Err("no key".into()),
        }
    }
}

fn discovered(providers: &ModelProviders, local: Vec<domain::ModelOption>) -> (Vec<String>, Vec<Unsolicited>) {
    let (outbox, rx) = outbox();
    providers.discover_models(local, &outbox);
    let mut told = Vec::new();
    loop {
        match sent(&rx) {
            crate::api::ServerEvent::Reply { id: RequestId(1), reply: Reply::Models(models) } => {
                return (models.into_iter().map(|m| m.id).collect(), told);
            }
            crate::api::ServerEvent::Unsolicited(unsolicited) => told.push(unsolicited),
            other => panic!("expected the models, got {other:?}"),
        }
    }
}

#[test]
fn every_listing_asked_for_is_answered_even_when_a_newer_one_was_asked_meanwhile() {
    let providers = rig().providers.with_catalog(Arc::new(ByKey));
    let (tx, rx) = crossbeam_channel::unbounded();
    let outbox = Outbox::new(tx, Arc::new(|| {}));
    providers.discover_models(Vec::new(), &outbox.answering(RequestId(1)));
    providers.discover_models(Vec::new(), &outbox.answering(RequestId(2)));
    let mut answered: Vec<_> = (0..2)
        .map(|_| match sent(&rx) {
            crate::api::ServerEvent::Reply { id, reply: Reply::Models(_) } => id,
            other => panic!("expected the models, got {other:?}"),
        })
        .collect();
    answered.sort();
    assert_eq!(answered, [RequestId(1), RequestId(2)]);
}

#[test]
fn each_connected_provider_lists_its_models_and_the_engine_s_come_last() {
    let providers = rig().providers.with_catalog(Arc::new(ByKey));
    assert!(discovered(&providers, Vec::new()).0.is_empty(), "no fixed list without an account");
    providers.connect(ModelProvider::new("keyed-too"), "sk-ant-0123456789");
    providers.connect(ModelProvider::new("keyed"), "sk-broken-0123456789");
    let fixture = domain::ModelOption::new("fixture", "Fixture").served_by(ModelProvider::new("keyed"));
    let (models, events) = discovered(&providers, vec![fixture]);
    assert_eq!(models, ["keyed-too:sk-ant", "fixture"]);
    assert!(events.iter().any(|e| matches!(
        e,
        Unsolicited::CatalogFailed { provider, .. } if *provider == ModelProvider::new("keyed")
    )));
}

#[path = "port_tests.rs"]
mod port;
