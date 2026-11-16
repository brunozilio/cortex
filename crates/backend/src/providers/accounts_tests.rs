use std::sync::Arc;

use domain::provider::ProviderAccount;
use protocol::ProviderEvent;

use super::*;
use crate::providers::InMemoryProviderStore;
use crate::secrets::{InMemorySecretStore, SecretStore};

const OWNER: Principal = Principal::LocalOwner;
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
    let key = providers.key(&OWNER, KEYED).expect("key");
    Config::parse(agent_engine::providers::get(KEYED).expect("known"), key.current().expect("load").secret())
        .expect("config")
        .api_key
}

fn accounts(event: &ServerEvent) -> Vec<ProviderAccount> {
    match event {
        ServerEvent::Providers(ProviderEvent::Listed { providers }) => {
            providers.iter().find(|p| p.provider == KEYED).map(|p| p.accounts.clone()).unwrap_or_default()
        }
        other => panic!("expected the list, got {other:?}"),
    }
}

#[test]
fn a_second_key_is_a_second_account_and_the_one_in_use() {
    let (providers, _) = rig();
    providers.connect(&OWNER, KEYED, FIRST);
    let event = providers.connect(&OWNER, KEYED, SECOND);
    let labels: Vec<String> = accounts(&event).into_iter().map(|a| a.label).collect();
    assert_eq!(labels, ["…aaaa", "…bbbb"]);
    assert_eq!(in_use(&providers), SECOND);
}

#[test]
fn switching_accounts_changes_the_key_the_next_run_gets() {
    let (providers, _) = rig();
    providers.connect(&OWNER, KEYED, FIRST);
    providers.connect(&OWNER, KEYED, SECOND);
    providers.use_account(&OWNER, KEYED, "1");
    assert_eq!(in_use(&providers), FIRST);
    providers.use_account(&OWNER, KEYED, "…bbbb");
    assert_eq!(in_use(&providers), SECOND);
    assert!(matches!(providers.use_account(&OWNER, KEYED, "9"), ServerEvent::Providers(ProviderEvent::Failed { .. })));
}

#[test]
fn the_same_key_again_keeps_one_account() {
    let (providers, _) = rig();
    providers.connect(&OWNER, KEYED, FIRST);
    assert_eq!(accounts(&providers.connect(&OWNER, KEYED, FIRST)).len(), 1);
}

#[test]
fn removing_an_account_forgets_only_its_secret() {
    let (providers, secrets) = rig();
    providers.connect(&OWNER, KEYED, FIRST);
    providers.connect(&OWNER, KEYED, SECOND);
    let second = accounts(&providers.list(&OWNER)).remove(1);
    let event = providers.remove_account(&OWNER, KEYED, "2");
    assert_eq!(accounts(&event).len(), 1);
    assert!(secrets.get(&OWNER, slot(KEYED, &second.id)).is_none());
    assert_eq!(in_use(&providers), FIRST);
}

#[test]
fn disconnecting_forgets_every_account_of_the_provider() {
    let (providers, secrets) = rig();
    providers.connect(&OWNER, KEYED, FIRST);
    providers.connect(&OWNER, KEYED, SECOND);
    let second = accounts(&providers.list(&OWNER)).remove(1);
    assert!(accounts(&providers.disconnect(&OWNER, KEYED)).is_empty());
    assert!(secrets.get(&OWNER, slot(KEYED, "")).is_none());
    assert!(secrets.get(&OWNER, slot(KEYED, &second.id)).is_none());
}

#[test]
fn a_subscription_account_is_named_by_its_email() {
    let (providers, _) = rig();
    let raw = |email: &str| {
        format!(r#"{{"access_token":"a","refresh_token":"r","expires_at":"2099-01-01T00:00:00Z","email":"{email}"}}"#)
    };
    let provider = ModelProvider::new("device-sub");
    providers.connect(&OWNER, provider, &raw("a@x.io"));
    let event = providers.connect(&OWNER, provider, &raw("b@x.io"));
    let ServerEvent::Providers(ProviderEvent::Listed { providers: listed }) = event else { panic!("listed") };
    let summary = listed.into_iter().find(|p| p.provider == provider).expect("device-sub");
    let labels: Vec<String> = summary.accounts.into_iter().map(|a| a.label).collect();
    assert_eq!(labels, ["a@x.io", "b@x.io"]);
}
