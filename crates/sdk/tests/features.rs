#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use backend::{InMemorySecretStore, LocalServices};
use sdk::protocol::{FeatureError, FeatureName};
use sdk::{Error, Model, Sdk};

fn sdk(data: &std::path::Path, off: &[&str]) -> Sdk {
    let engine = FakeEngine::new(vec![Model::new("scripted", "Scripted")], Vec::new());
    let mut services = LocalServices::with_engine(Arc::new(engine));
    services.off.extend(off.iter().map(|feature| (*feature).to_owned()));
    services.homes = Arc::new(backend::OwnerHome::at(data));
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    Sdk::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

#[test]
fn a_feature_that_is_turned_off_is_an_error_of_its_own_that_says_how_to_turn_it_on() {
    let data = tempfile::tempdir().expect("data");
    let sdk = sdk(data.path(), &["memory_search", "suggested_rules"]);

    let refused = sdk.suggested_rules(data.path()).expect_err("off");

    assert!(
        matches!(&refused, Error::Feature(FeatureError::FeatureOff { feature: FeatureName::SuggestedRules })),
        "{refused:?}"
    );
    assert_eq!(refused.to_string(), "suggested_rules is turned off (/features suggested-rules on)");
    assert!(sdk.notices().expect("notices").is_empty());
}

#[test]
fn a_feature_that_fails_says_why_and_names_itself() {
    let data = tempfile::tempdir().expect("data");
    let sdk = sdk(data.path(), &["memory_search"]);

    let failed = sdk.background_output("bash_none").expect_err("no such command");

    assert!(
        matches!(&failed, Error::Feature(FeatureError::Failed { feature: FeatureName::Shell, message })
            if message == "no background command bash_none"),
        "{failed:?}"
    );
}

#[test]
fn each_feature_answers_its_typed_call() {
    let data = tempfile::tempdir().expect("data");
    let sdk = sdk(data.path(), &["memory_search"]);
    let cwd = data.path();

    assert!(sdk.sessions().expect("sessions").is_empty());
    assert!(sdk.memory(cwd).list().expect("memory").is_empty());
    assert!(sdk.project_files(cwd).expect("files").is_empty());
    assert_eq!(sdk.with_mentioned_files(cwd, "no files here").expect("mentions"), "no files here");
    assert!(sdk.git_offered(cwd).expect("offer"));
    assert!(sdk.background_commands().expect("commands").is_empty());
    let plugins = sdk.plugins(sdk::PluginsRequest::List).expect("plugins");
    assert_eq!(plugins, sdk::PluginsReply::Listed(Vec::new()));
    assert!(sdk.flags(cwd).expect("flags").iter().any(|listed| listed.flag.name == "budget" && listed.flag.on));
    assert!(sdk.scheduled().expect("scheduled").is_empty());
    assert!(sdk.take_due_schedules().expect("due").is_empty());
    assert!(sdk.suggested_rules(cwd).expect("rules").is_empty());
    assert_eq!(sdk.trace_of_every_session().expect("trace")["sessions"], serde_json::json!([]));
    assert_eq!(sdk.open_channels(cwd, &[]).expect("channels"), (Vec::new(), Vec::new()));
    assert_eq!(sdk.code_map(cwd, "hello").expect("code map"), None);
    assert!(sdk.declare_providers(cwd).is_ok());
    assert!(sdk.output_savings().expect("savings").is_empty());
}
