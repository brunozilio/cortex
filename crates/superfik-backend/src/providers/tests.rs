use std::sync::Arc;

use superfik_core::provider::{ModelProvider, ProviderSummary};

use super::*;
use crate::plugins::InMemorySecretStore;

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
    let key = rig.providers.key(&OWNER, ModelProvider::OpenAi).expect("key");
    assert_eq!(key.secret(), "sk-proj-0123456789abcd");
}

#[test]
fn a_blank_key_fails_and_keeps_nothing() {
    let rig = rig();
    assert_eq!(failure(&rig.providers.connect(&OWNER, ModelProvider::Anthropic, "   ")), "the key is empty");
    assert!(rig.secrets.get(&OWNER, secret_id(ModelProvider::Anthropic)).is_none());
    let missing = rig.providers.key(&OWNER, ModelProvider::Anthropic).expect_err("no key");
    assert!(missing.contains("Settings"), "{missing}");
}

#[test]
fn a_local_model_takes_no_key() {
    let rig = rig();
    let event = rig.providers.connect(&OWNER, ModelProvider::Local, "sk-proj-0123456789abcd");
    assert_eq!(failure(&event), "local models need no key");
    assert!(rig.secrets.get(&OWNER, secret_id(ModelProvider::Local)).is_none());
}

#[test]
fn a_new_key_replaces_the_old_and_disconnecting_forgets_it() {
    let rig = rig();
    rig.providers.connect(&OWNER, ModelProvider::OpenAi, "sk-proj-0123456789abcd");
    rig.providers.connect(&OWNER, ModelProvider::OpenAi, "sk-proj-0123456789wxyz");
    assert_eq!(rig.providers.key(&OWNER, ModelProvider::OpenAi).expect("key").secret(), "sk-proj-0123456789wxyz");
    assert!(listed(&rig.providers.disconnect(&OWNER, ModelProvider::OpenAi))[0].account.is_none());
    assert!(rig.secrets.get(&OWNER, secret_id(ModelProvider::OpenAi)).is_none());
}

#[test]
fn owners_keep_their_providers_apart() {
    let rig = rig();
    rig.providers.connect(&OWNER, ModelProvider::OpenAi, "sk-proj-0123456789abcd");
    let member = Principal::member(superfik_core::UserId::new(), superfik_core::OrgId::new());
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
