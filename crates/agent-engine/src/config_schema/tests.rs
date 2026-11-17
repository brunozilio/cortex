use std::path::{Path, PathBuf};

use serde_json::json;

use super::*;

const UPDATE: &str = "UPDATE_SCHEMAS";

fn content_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content")
}

fn text_of(schema: &Value) -> String {
    serde_json::to_string_pretty(schema).expect("a schema serializes") + "\n"
}

#[test]
fn the_published_schemas_are_what_the_loaders_check() {
    let folder = content_folder();
    if std::env::var_os(UPDATE).is_some() {
        for (name, schema) in published() {
            let path = folder.join(name);
            std::fs::create_dir_all(path.parent().expect("a folder")).expect("the schemas folder");
            std::fs::write(path, text_of(&schema)).expect("a schema");
        }
        return;
    }
    for (name, schema) in published() {
        let committed = std::fs::read_to_string(folder.join(name)).unwrap_or_default();
        assert!(
            committed == text_of(&schema),
            "content/{name} is not what the loaders check now: run `cargo run -p xtask -- schemas`"
        );
    }
}

#[test]
fn each_published_schema_is_a_valid_json_schema() {
    for (name, schema) in published() {
        assert!(jsonschema::meta::is_valid(&schema), "{name}");
        jsonschema::validator_for(&schema).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[test]
fn the_published_schemas_never_spell_the_product() {
    for (name, schema) in published() {
        let text = text_of(&schema).to_ascii_lowercase();
        for word in [brand::NAME, brand::COMMAND] {
            assert!(!text.contains(&word.to_ascii_lowercase()), "{name} spells {word}");
        }
    }
}

#[test]
fn the_shipped_copy_names_the_app_s_own_folder() {
    let shipped = content::builtin_text(SETTINGS_FILE).expect("shipped with the app");
    let schema: Value = serde_json::from_str(&shipped).expect("JSON");
    assert!(shipped.contains(brand::DIR) && !shipped.contains("{{DIR}}"));
    assert_eq!(
        schema["properties"].as_object().map(|keys| keys.len()),
        settings()["properties"].as_object().map(|keys| keys.len())
    );
}

#[test]
fn a_wrong_value_is_named_by_its_path_and_left_out_and_the_rest_is_kept() {
    let checked =
        checked_settings(json!({ "fallbackModel": ["a", 3, "b"], "sandbox": { "network": "off", "required": true } }));

    assert_eq!(
        checked.problems,
        ["fallbackModel[1]: expected null or string, found 3", "sandbox.network: expected boolean, found \"off\""]
    );
    assert_eq!(checked.value, json!({ "fallbackModel": ["a", "b"], "sandbox": { "required": true } }));
    assert_eq!(
        checked.located(Path::new("/home/me/settings.json"))[0],
        "/home/me/settings.json: fallbackModel[1]: expected null or string, found 3"
    );
}

#[test]
fn every_wrong_item_of_a_list_is_left_out_from_the_last_back() {
    let checked = checked_settings(json!({ "permissions": { "deny": [1, "Read(.env)", 2] } }));

    assert_eq!(checked.problems.len(), 2, "{:?}", checked.problems);
    assert_eq!(checked.value, json!({ "permissions": { "deny": ["Read(.env)"] } }));
}

#[test]
fn an_unknown_key_is_named_once_and_left_out() {
    let checked = checked_settings(
        json!({ "$schema": "./schemas/settings.schema.json", "theme": "dark", "scripts": { "disk": true } }),
    );

    assert_eq!(checked.problems, ["scripts.disk: unknown key, ignored", "theme: unknown key, ignored"]);
    assert_eq!(checked.value, json!({ "$schema": "./schemas/settings.schema.json", "scripts": {} }));
}

#[test]
fn a_feature_the_app_does_not_list_is_still_a_switch() {
    let checked = checked_settings(json!({ "features": { "insights": false, "trace": "no" } }));

    assert_eq!(checked.problems, ["features.trace: expected boolean, found \"no\""]);
    assert_eq!(checked.value, json!({ "features": { "insights": false } }));
}

#[test]
fn a_hook_without_its_type_is_left_out_whole() {
    let checked = checked_settings(
        json!({ "hooks": { "Stop": [{ "hooks": [{ "command": "x" }, { "type": "command", "command": "y", "timeout": 0 }] }] } }),
    );

    assert_eq!(
        checked.problems,
        [
            "hooks.Stop[0].hooks[0]: expected the key \"type\"",
            "hooks.Stop[0].hooks[1].timeout: expected at least 1, found 0"
        ]
    );
    assert_eq!(checked.value, json!({ "hooks": { "Stop": [{ "hooks": [] }] } }));
}

#[test]
fn an_mcp_server_with_a_wrong_value_is_left_out_whole_and_the_others_kept() {
    let config = json!({
        "$schema": "https://example.com/mcp.schema.json",
        "mcpServers": {
            "jira": { "type": "websocket", "url": "wss://jira" },
            "github": { "command": "github-mcp", "args": ["--stdio"], "sandbox": { "network": "yes" } },
            "docs": { "url": "https://docs.example.com/mcp", "timeout": 30 }
        }
    });

    let checked = checked_mcp(config);

    assert_eq!(
        checked.problems,
        [
            "mcpServers.docs.timeout: unknown key, ignored",
            "mcpServers.github.sandbox.network: expected boolean, found \"yes\"",
            "mcpServers.jira.type: expected one of \"stdio\", \"http\", \"sse\", found \"websocket\"",
        ]
    );
    assert_eq!(
        checked.value["mcpServers"],
        json!({ "docs": { "url": "https://docs.example.com/mcp" } }),
        "an unknown key is only ignored; a wrong value could start the server unlike its author meant"
    );
}

#[test]
fn a_file_that_is_not_an_object_is_left_out_whole() {
    let checked = checked_mcp(json!(["jira"]));

    assert_eq!(checked.problems, ["expected object, found [\"jira\"]"]);
    assert_eq!(checked.value, json!({}));
}

#[test]
fn a_key_with_a_slash_or_a_number_is_found_by_its_name() {
    let checked = checked_settings(json!({ "lspServers": { "a/b": { "args": 1 }, "7": { "disabled": 1 } } }));

    assert_eq!(
        checked.problems,
        ["lspServers.7.disabled: expected boolean, found 1", "lspServers.a/b.args: expected array or null, found 1"]
    );
    assert_eq!(checked.value, json!({ "lspServers": { "a/b": {}, "7": {} } }));
}

#[test]
fn a_long_value_is_shown_cut_short() {
    let long = "x".repeat(200);
    let checked = checked_settings(json!({ "agent": [long] }));

    let problem = &checked.problems[0];
    assert!(problem.starts_with("agent: expected null or string, found [\"xxx") && problem.ends_with('…'), "{problem}");
    assert!(problem.chars().count() < 120);
}

#[test]
fn a_problem_is_logged_once() {
    let problem = "s.json: theme: unknown key, ignored (logged once)".to_owned();
    assert_eq!(logged_once(std::slice::from_ref(&problem)), 1);
    assert_eq!(logged_once(&[problem]), 0, "told already");
}
