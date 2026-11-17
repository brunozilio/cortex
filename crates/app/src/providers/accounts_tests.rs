use std::sync::Arc;

use crate::api::ProviderEvent;
use domain::provider::ProviderAccount;

use super::*;
use crate::providers::InMemoryProviderStore;
use crate::secrets::{InMemorySecretStore, SecretStore};

const KEYED: ModelProvider = ModelProvider::new("keyed");
const FIRST: &str = "sk-proj-0123456789aaaa";
const SECOND: &str = "sk-proj-0123456789bbbb";

fn rig() -> (ModelProviders, Arc<InMemorySecretStore>) {
    llm_providers::testing::install();
    let secrets = Arc::new(InMemorySecretStore::default());
    let providers =
        ModelProviders::new(Arc::new(InMemoryProviderStore::default()), secrets.clone()).with_clock(|| 1_000);
    (providers, secrets)
}

fn in_use(providers: &ModelProviders) -> String {
    let key = providers.key(KEYED).expect("key");
    Config::parse(agent_engine::providers::get(KEYED).expect("known"), key.current().expect("load").secret())
        .expect("config")
        .api_key
}

fn accounts(event: &Reply) -> Vec<ProviderAccount> {
    match event {
        Reply::Providers(ProviderEvent::Listed { providers }) => {
            providers.iter().find(|p| p.provider == KEYED).map(|p| p.accounts.clone()).unwrap_or_default()
        }
        other => panic!("expected the list, got {other:?}"),
    }
}

#[test]
fn a_second_key_is_a_second_account_and_the_one_in_use() {
    let (providers, _) = rig();
    providers.connect(KEYED, FIRST);
    let event = providers.connect(KEYED, SECOND);
    let labels: Vec<String> = accounts(&event).into_iter().map(|a| a.label).collect();
    assert_eq!(labels, ["…aaaa", "…bbbb"]);
    assert_eq!(in_use(&providers), SECOND);
}

#[test]
fn switching_accounts_changes_the_key_the_next_run_gets() {
    let (providers, _) = rig();
    providers.connect(KEYED, FIRST);
    providers.connect(KEYED, SECOND);
    providers.use_account(KEYED, "1");
    assert_eq!(in_use(&providers), FIRST);
    providers.use_account(KEYED, "…bbbb");
    assert_eq!(in_use(&providers), SECOND);
    assert!(matches!(providers.use_account(KEYED, "9"), Reply::Providers(ProviderEvent::Failed { .. })));
}

#[test]
fn the_same_key_again_keeps_one_account() {
    let (providers, _) = rig();
    providers.connect(KEYED, FIRST);
    assert_eq!(accounts(&providers.connect(KEYED, FIRST)).len(), 1);
}

#[test]
fn removing_an_account_forgets_only_its_secret() {
    let (providers, secrets) = rig();
    providers.connect(KEYED, FIRST);
    providers.connect(KEYED, SECOND);
    let second = accounts(&providers.list()).remove(1);
    let event = providers.remove_account(KEYED, "2");
    assert_eq!(accounts(&event).len(), 1);
    assert!(secrets.get(slot(KEYED, &second.id)).is_none());
    assert_eq!(in_use(&providers), FIRST);
}

#[test]
fn disconnecting_forgets_every_account_of_the_provider() {
    let (providers, secrets) = rig();
    providers.connect(KEYED, FIRST);
    providers.connect(KEYED, SECOND);
    let second = accounts(&providers.list()).remove(1);
    assert!(accounts(&providers.disconnect(KEYED)).is_empty());
    assert!(secrets.get(slot(KEYED, "")).is_none());
    assert!(secrets.get(slot(KEYED, &second.id)).is_none());
}

#[test]
fn a_subscription_account_is_named_by_its_email() {
    let (providers, _) = rig();
    let raw = |email: &str| {
        format!(r#"{{"access_token":"a","refresh_token":"r","expires_at":"2099-01-01T00:00:00Z","email":"{email}"}}"#)
    };
    let provider = ModelProvider::new("device-sub");
    providers.connect(provider, &raw("a@x.io"));
    let event = providers.connect(provider, &raw("b@x.io"));
    let Reply::Providers(ProviderEvent::Listed { providers: listed }) = event else { panic!("listed") };
    let summary = listed.into_iter().find(|p| p.provider == provider).expect("device-sub");
    let labels: Vec<String> = summary.accounts.into_iter().map(|a| a.label).collect();
    assert_eq!(labels, ["a@x.io", "b@x.io"]);
}

fn order(providers: &ModelProviders) -> Vec<String> {
    let keys = providers.keys(KEYED).expect("keys");
    keys.into_iter()
        .map(|(_, key)| {
            let config = Config::parse(agent_engine::providers::get(KEYED).expect("known"), key.secret());
            config.expect("config").api_key
        })
        .collect()
}

#[test]
fn without_a_balance_a_run_gets_only_the_account_in_use() {
    let (providers, _) = rig();
    providers.connect(KEYED, FIRST);
    providers.connect(KEYED, SECOND);
    assert_eq!(order(&providers), [SECOND]);
}

#[test]
fn failover_offers_the_account_in_use_first_and_the_others_after() {
    let (providers, _) = rig();
    providers.connect(KEYED, FIRST);
    providers.connect(KEYED, SECOND);
    providers.set_balance(KEYED, domain::account_pool::Balance::Failover);
    assert_eq!(order(&providers), [SECOND, FIRST]);
    assert_eq!(order(&providers), [SECOND, FIRST]);
}

#[test]
fn round_robin_starts_each_run_on_the_next_account() {
    let (providers, _) = rig();
    providers.connect(KEYED, FIRST);
    providers.connect(KEYED, SECOND);
    let event = providers.set_balance(KEYED, domain::account_pool::Balance::RoundRobin);
    let Reply::Providers(ProviderEvent::Listed { providers: listed }) = event else { panic!("listed") };
    assert!(listed.iter().any(|p| p.provider == KEYED && p.balance == domain::account_pool::Balance::RoundRobin));
    assert_eq!(order(&providers), [FIRST, SECOND]);
    assert_eq!(order(&providers), [SECOND, FIRST]);
    assert_eq!(order(&providers), [FIRST, SECOND]);
}

#[test]
fn without_an_email_the_token_s_subject_keeps_two_accounts_apart() {
    let (providers, _) = rig();
    let raw = |subject: &str| {
        format!(
            r#"{{"access_token":"a","refresh_token":"r","expires_at":"2099-01-01T00:00:00Z","subject":"{subject}"}}"#
        )
    };
    let provider = ModelProvider::new("device-sub");
    providers.connect(provider, &raw("0f3a9c1e-user-one"));
    providers.connect(provider, &raw("7b22d410-user-two"));
    let event = providers.connect(provider, &raw("0f3a9c1e-user-one"));
    let Reply::Providers(ProviderEvent::Listed { providers: listed }) = event else { panic!("listed") };
    let summary = listed.into_iter().find(|p| p.provider == provider).expect("device-sub");
    let labels: Vec<String> = summary.accounts.into_iter().map(|a| a.label).collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert!(labels[0].ends_with("0f3a9c1e") && labels[1].ends_with("7b22d410"), "{labels:?}");
}

#[test]
fn a_sign_in_that_names_no_one_is_added_as_a_numbered_account_and_never_replaces_one() {
    let (providers, _) = rig();
    let raw = r#"{"access_token":"a","refresh_token":"r","expires_at":"2099-01-01T00:00:00Z"}"#;
    let provider = ModelProvider::new("device-sub");
    providers.connect(provider, raw);
    let event = providers.connect(provider, raw);
    let Reply::Providers(ProviderEvent::Listed { providers: listed }) = event else { panic!("listed") };
    let summary = listed.into_iter().find(|p| p.provider == provider).expect("device-sub");
    let labels: Vec<String> = summary.accounts.into_iter().map(|a| a.label).collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert!(labels[1].ends_with("#2"), "{labels:?}");
}
