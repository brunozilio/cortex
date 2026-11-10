use super::*;

const LINEAR: &str = r#"{
  "id": "linear", "name": "Linear", "version": "1.0.0", "icon": "issue",
  "accounts": "many",
  "auth": { "kind": "oauth" },
  "mcp": [{ "transport": "http", "name": "linear", "url": "https://mcp.linear.app/mcp",
            "headers": { "Authorization": "Bearer {{secret.access_token}}" } }],
  "instructions": "Linear workspace {{account}}: use the mcp__{{server}}__* tools."
}"#;

fn with(json: &str, from: &str, to: &str) -> Result<PluginManifest, PluginError> {
    PluginManifest::parse(&json.replace(from, to))
}

#[test]
fn parses_a_full_manifest() {
    let manifest = PluginManifest::parse(LINEAR).expect("valid");
    assert_eq!(manifest.id, PluginId("linear".into()));
    assert_eq!(manifest.accounts, Accounts::Many);
    assert_eq!(manifest.auth.kind(), AuthKind::OAuth);
    assert_eq!(manifest.first_http_url(), Some("https://mcp.linear.app/mcp"));
    assert_eq!(manifest.mcp[0].name(), "linear");
}

#[test]
fn defaults_make_a_minimal_manifest() {
    let manifest = PluginManifest::parse(r#"{"id":"notes","name":"Notes","version":"0.1"}"#).expect("valid");
    assert_eq!(manifest.auth, Auth::None);
    assert_eq!(manifest.accounts, Accounts::One);
    assert!(manifest.mcp.is_empty());
}

#[test]
fn api_key_fields_become_template_secrets() {
    let json = r#"{"id":"c7","name":"Context7","version":"1",
        "auth":{"kind":"api_key","fields":[{"key":"api_key","label":"API key"}]},
        "mcp":[{"transport":"stdio","name":"c7","command":"npx","args":["-y","c7"],
                "env":{"KEY":"{{secret.api_key}}"}}]}"#;
    let manifest = PluginManifest::parse(json).expect("valid");
    assert_eq!(manifest.auth.secret_keys(), vec!["api_key"]);
    assert_eq!(manifest.auth.fields()[0].label, "API key");
    assert!(with(json, "secret.api_key", "secret.other").is_err(), "unknown secret");
}

#[test]
fn rejects_broken_manifests() {
    assert!(with(LINEAR, r#""id": "linear""#, r#""id": "Linear!""#).is_err(), "id not a slug");
    assert!(with(LINEAR, r#""name": "Linear""#, r#""name": " ""#).is_err(), "empty name");
    assert!(with(LINEAR, r#""kind": "oauth""#, r#""kind": "none""#).is_err(), "many accounts without auth");
    assert!(with(LINEAR, "{{account}}", "{{who}}").is_err(), "unknown placeholder");
    assert!(with(LINEAR, "{{server}}", "{{server").is_err(), "unclosed placeholder");
    assert!(with(LINEAR, r#""name": "linear""#, r#""name": "Lin ear""#).is_err(), "server not a slug");
    assert!(PluginManifest::parse("{").is_err());
    let twice =
        LINEAR.replace(r#""mcp": ["#, r#""mcp": [{ "transport": "http", "name": "linear", "url": "https://x" }, "#);
    assert!(PluginManifest::parse(&twice).is_err(), "duplicate server");
    let oauth_without_http = r#"{"id":"x","name":"X","version":"1","auth":{"kind":"oauth"}}"#;
    assert!(PluginManifest::parse(oauth_without_http).is_err());
}
