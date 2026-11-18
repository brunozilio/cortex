use serde_json::json;

use super::flags::FlagsRequest;
use super::memory::{MemoryOp, MemoryRequest};
use super::plugins::PluginsRequest;
use super::schedules::SchedulesRequest;
use super::sessions::SessionsRequest;
use super::shell::ShellRequest;
use super::suggested_rules::SuggestedRulesRequest;
use super::trace::TraceRequest;
use super::workspace::{WorkspaceOp, WorkspaceRequest};
use super::*;

fn refused<T: serde::de::DeserializeOwned>(json: serde_json::Value) -> bool {
    serde_json::from_value::<T>(json).is_err()
}

#[test]
fn every_feature_has_a_name_in_one_fixed_order() {
    let names: Vec<&str> = FeatureName::ALL.iter().map(|feature| feature.name()).collect();
    assert_eq!(
        names,
        [
            "channels",
            "code_map",
            "declared_providers",
            "flags",
            "memory",
            "output_savings",
            "plugins",
            "schedules",
            "sessions",
            "shell",
            "suggested_rules",
            "trace",
            "workspace",
        ]
    );
    for feature in FeatureName::ALL {
        assert_eq!(serde_json::to_value(feature).expect("json"), json!(feature.name()));
    }
}

#[test]
fn a_request_names_its_feature_on_the_wire() {
    let request =
        FeatureRequest::Sessions(SessionsRequest::Rename { session: domain::SessionId("s".into()), title: "t".into() });
    let json = serde_json::to_value(&request).expect("json");
    assert_eq!(json, json!({ "feature": "sessions", "request": { "rename": { "session": "s", "title": "t" } } }));
    assert_eq!(request.feature(), FeatureName::Sessions);
    assert_eq!(
        serde_json::to_value(FeatureRequest::OutputSavings).expect("json"),
        json!({ "feature": "output_savings" })
    );
    assert!(refused::<FeatureRequest>(json!({ "feature": "sesions", "request": "list" })), "a typo in the feature");
}

#[test]
fn what_used_to_take_a_silent_default_is_refused_when_it_is_missing() {
    assert!(refused::<SessionsRequest>(json!({ "rename": { "session": "s" } })), "the title is required");
    assert!(refused::<SessionsRequest>(json!({ "archive": { "session": "s", "archived": true } })));
    assert!(refused::<MemoryRequest>(json!({ "path": "/repo" })), "the op is required");
    assert!(refused::<MemoryRequest>(json!({ "path": "/repo", "op": { "search": {} } })), "the query is required");
    assert!(refused::<MemoryRequest>(json!({ "path": "/repo", "op": "list", "pth": "/x" })), "an unknown field");
    assert!(refused::<WorkspaceRequest>(json!({ "path": "/repo", "op": { "mentions": {} } })));
    assert!(refused::<PluginsRequest>(json!({})), "the op is required");
    assert!(refused::<FlagsRequest>(json!({ "set": { "cwd": "/repo", "name": "budget" } })), "the state is required");
    assert!(refused::<FlagsRequest>(json!({ "set": { "cwd": "/repo", "on": false } })), "the name is required");
    assert!(refused::<SuggestedRulesRequest>(json!({ "keep_rule": { "cwd": "/repo", "rule": "Bash(ls:*)" } })));
    assert!(refused::<SuggestedRulesRequest>(json!({ "keep_rule": { "cwd": "/r", "rule": "x", "kind": "dney" } })));
    assert!(refused::<TraceRequest>(json!({ "session": { "session": "s" } })), "the folder is required");
    assert!(refused::<SchedulesRequest>(json!({ "create": { "prompt": "x" } })));
    assert!(refused::<ShellRequest>(json!({ "run": { "command": "ls" } })), "the folder is required");
}

#[test]
fn every_request_round_trips() {
    let requests = [
        FeatureRequest::Memory(MemoryRequest { path: "/repo".into(), op: MemoryOp::Search { query: "x".into() } }),
        FeatureRequest::Workspace(WorkspaceRequest { path: "/repo".into(), op: WorkspaceOp::GitOffer }),
        FeatureRequest::Shell(ShellRequest::Kill { id: "bash_1".into() }),
        FeatureRequest::Plugins(PluginsRequest::Install { source: "git@x:a/b.git".into(), name: None }),
        FeatureRequest::Flags(FlagsRequest::Set { cwd: "/repo".into(), name: "budget".into(), on: false }),
        FeatureRequest::Schedules(SchedulesRequest::TakeDue),
        FeatureRequest::Trace(TraceRequest::EverySession),
        FeatureRequest::OutputSavings,
    ];
    for request in requests {
        let json = serde_json::to_string(&request).expect("serialize");
        assert_eq!(serde_json::from_str::<FeatureRequest>(&json).expect("deserialize"), request, "{json}");
    }
}

#[test]
fn a_feature_that_is_off_says_how_to_turn_it_on_and_a_failure_says_what_failed() {
    let off = FeatureError::FeatureOff { feature: FeatureName::SuggestedRules };
    assert_eq!(off.to_string(), "suggested_rules is turned off (/features suggested-rules on)");
    for feature in FeatureName::ALL {
        let said = FeatureError::FeatureOff { feature }.to_string();
        match feature.switch() {
            Some(flag) => {
                assert!(domain::features::known(flag).is_some(), "/features refuses {flag}, the switch of {feature}");
                assert_eq!(said, format!("{feature} is turned off (/features {flag} on)"));
            }
            None => assert_eq!(said, format!("{feature} is turned off"), "no command is named that would refuse"),
        }
    }
    let failed = FeatureError::Failed { feature: FeatureName::Memory, message: "nothing to undo".into() };
    assert_eq!((failed.to_string().as_str(), failed.feature()), ("nothing to undo", FeatureName::Memory));
    assert_ne!(off, FeatureError::Failed { feature: FeatureName::SuggestedRules, message: off.to_string() });
}
