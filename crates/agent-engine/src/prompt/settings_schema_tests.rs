use serde_json::{Value, json};

use super::load_trusted as load;

fn folders() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("repo"))
}

fn write(root: &std::path::Path, name: &str, settings: &Value) -> std::path::PathBuf {
    let folder = root.join(brand::DIR);
    std::fs::create_dir_all(&folder).expect("dir");
    let path = folder.join(name);
    std::fs::write(&path, settings.to_string()).expect("write");
    path
}

#[test]
fn a_key_of_the_wrong_type_is_reported_and_the_rest_of_the_file_still_applies() {
    let (home, repo) = folders();
    let path = write(
        repo.path(),
        "settings.json",
        &json!({
            "enableAllProjectMcpServers": "no",
            "outputStyle": "Concise",
            "permissions": { "deny": ["Bash(rm:*)", 5] }
        }),
    );

    let settings = load(repo.path(), Some(home.path()), &[]).expect("the run goes on");

    let shown = path.display().to_string();
    assert!(
        settings.notices.contains(&format!("{shown}: enableAllProjectMcpServers: expected boolean, found \"no\"")),
        "{:?}",
        settings.notices
    );
    assert!(settings.notices.contains(&format!("{shown}: permissions.deny[1]: expected string, found 5")));
    assert!(!settings.mcp_approval.all, "the wrong value is left out");
    assert_eq!(settings.style.map(|style| style.name), Some("Concise".into()));
    let cwd = repo.path().to_string_lossy().into_owned();
    let target = domain::permission_rules::Target::Command("rm -rf x");
    let denied = settings.permissions.decide("Bash", &target, &cwd, None);
    assert_eq!(denied, Some(domain::permission_rules::RuleKind::Deny), "the good rule beside the bad one holds");
}

#[test]
fn a_value_outside_its_choices_is_reported_with_the_choices() {
    let (home, repo) = folders();
    let hooks = json!({ "hooks": { "Stop": [
        { "hooks": [{ "type": "shell", "command": "true" }] },
        { "hooks": [{ "type": "command", "command": "echo kept" }] }
    ] } });
    let path = write(home.path(), "settings.json", &hooks);

    let settings = load(repo.path(), Some(home.path()), &[]).expect("the run goes on");

    let expected = format!(
        "{}: hooks.Stop[0].hooks[0].type: expected one of \"command\", \"script\", \"http\", \"prompt\", \"agent\", \
         found \"shell\"",
        path.display()
    );
    assert!(settings.notices.contains(&expected), "{:?}", settings.notices);
    let listed = settings.hooks.listed();
    assert_eq!(listed.len(), 1, "{listed:?}");
}

#[test]
fn an_unknown_key_is_named_once_and_ignored() {
    let (home, repo) = folders();
    let path = write(
        home.path(),
        "settings.json",
        &json!({ "outputStyle": "Concise", "theme": "dark", "permissions": { "defaultMode": "plan" } }),
    );

    let settings = load(repo.path(), Some(home.path()), &[]).expect("the run goes on");

    let shown = path.display().to_string();
    let about = |key: &str| settings.notices.iter().filter(|notice| notice.contains(key)).count();
    assert_eq!(about("theme"), 1, "{:?}", settings.notices);
    assert!(settings.notices.contains(&format!("{shown}: theme: unknown key, ignored")));
    assert!(settings.notices.contains(&format!("{shown}: permissions.defaultMode: unknown key, ignored")));
    assert_eq!(settings.style.map(|style| style.name), Some("Concise".into()));
}

#[test]
fn a_leftover_auto_memory_switch_is_named_once_and_ignored() {
    let (home, repo) = folders();
    let path = write(repo.path(), "settings.json", &json!({ "autoMemoryEnabled": false, "outputStyle": "Concise" }));

    let settings = load(repo.path(), Some(home.path()), &[]).expect("the file still loads");

    let about = settings.notices.iter().filter(|notice| notice.contains("autoMemoryEnabled")).count();
    assert_eq!(about, 1, "{:?}", settings.notices);
    assert!(settings.notices.contains(&format!("{}: autoMemoryEnabled: unknown key, ignored", path.display())));
    assert!(settings.skipped_files.is_empty(), "{:?}", settings.skipped_files);
    assert_eq!(settings.style.map(|style| style.name), Some("Concise".into()));
}

#[test]
fn an_unknown_hook_event_is_named_and_the_other_hooks_still_run() {
    let (home, repo) = folders();
    let hooks = json!({ "hooks": {
        "OnSave": [{ "hooks": [{ "type": "command", "command": "true" }] }],
        "Stop": [{ "hooks": [{ "type": "command", "command": "echo kept" }] }]
    } });
    let path = write(home.path(), "settings.json", &hooks);

    let settings = load(repo.path(), Some(home.path()), &[]).expect("the run goes on");

    assert!(settings.notices.contains(&format!("{}: hooks.OnSave: unknown key, ignored", path.display())));
    assert_eq!(settings.hooks.listed().len(), 1);
}

#[test]
fn a_file_that_is_not_an_object_is_reported_and_skipped() {
    let (home, repo) = folders();
    let path = write(repo.path(), "settings.json", &json!(["outputStyle", "Concise"]));
    write(home.path(), "settings.json", &json!({ "outputStyle": "Concise" }));

    let settings = load(repo.path(), Some(home.path()), &[]).expect("the run goes on");

    assert!(settings.notices.iter().any(|notice| notice.starts_with(&format!("{}: expected object", path.display()))));
    assert_eq!(settings.style.map(|style| style.name), Some("Concise".into()));
}

#[test]
fn a_schema_reference_is_not_an_unknown_key() {
    let (home, repo) = folders();
    let schema = json!({ "$schema": "./schemas/settings.schema.json", "outputStyle": "Concise" });
    write(home.path(), "settings.json", &schema);
    write(repo.path(), "settings.local.json", &schema);

    let settings = load(repo.path(), Some(home.path()), &[]).expect("settings");

    assert!(settings.notices.is_empty(), "{:?}", settings.notices);
}

#[test]
fn the_documented_full_example_reads_without_a_notice() {
    let (home, repo) = folders();
    let example = json!({
        "outputStyle": "explanatory",
        "advisorModel": "opus",
        "judgeModel": "sonnet",
        "smallModel": "haiku",
        "escalationModel": "opus",
        "fallbackModel": ["sonnet", "gpt-5"],
        "agent": null,
        "jsonSchema": null,
        "enabledProviders": ["acme-llm"],
        "enableAllProjectMcpServers": false,
        "enabledMcpServers": ["jira"],
        "disabledMcpServers": ["sonar"],
        "permissions": {
            "allow": ["Bash(cargo test:*)", "Edit(src/**)"],
            "ask": ["Bash(git push:*)"],
            "deny": ["Read(*.env)"],
            "additionalDirectories": ["~/notes"]
        },
        "verification": { "allow": ["Bash(cargo test:*)"] },
        "scripts": { "files": true, "processes": true, "network": false },
        "sandbox": {
            "network": true, "allowWrite": ["~/.m2"], "required": true,
            "allowUnixSockets": ["~/.docker/run/docker.sock"], "allowRead": ["~/.ssh/known_hosts"]
        },
        "features": { "experimental-code-map": false, "git-offer": false },
        "lspServers": {
            "zls": { "command": "zls", "args": [], "extensionToLanguage": { ".zig": "zig" },
                     "rootMarkers": ["build.zig"], "env": { "A": "1" } },
            "clangd": { "disabled": true }
        },
        "hooks": {
            "PostToolUse": [{ "matcher": "Edit|Write", "hooks": [
                { "type": "command", "command": "cargo fmt", "timeout": 30, "status_message": "formatting" },
                { "type": "http", "url": "https://hooks.example.com/x", "headers": { "X-Token": "$HOOK_TOKEN" },
                  "allowed_env_vars": ["HOOK_TOKEN"] },
                { "type": "prompt", "prompt": "Check it", "model": "haiku" }
            ] }]
        }
    });
    write(home.path(), "settings.json", &example);

    let settings = load(repo.path(), Some(home.path()), &[]).expect("settings");

    assert!(settings.notices.is_empty(), "{:?}", settings.notices);
    assert_eq!(settings.sandbox.allow_unix_sockets, ["~/.docker/run/docker.sock"]);
}

fn one_value_per_published_key() -> Vec<(&'static str, Value)> {
    vec![
        ("outputStyle", json!("concise")),
        ("agent", json!("reviewer")),
        ("bashOutputLimit", json!(4_000)),
        ("advisorModel", json!("a")),
        ("judgeModel", json!("j")),
        ("smallModel", json!("s")),
        ("teammateMode", json!("tmux")),
        ("escalationModel", json!("e")),
        ("fallbackModel", json!(["f"])),
        ("jsonSchema", json!({ "type": "object" })),
        ("enabledProviders", json!(["acme"])),
        ("enableAllProjectMcpServers", json!(true)),
        ("enabledMcpServers", json!(["jira"])),
        ("disabledMcpServers", json!(["sonar"])),
        ("hooks", json!({ "Stop": [{ "hooks": [{ "type": "command", "command": "true" }] }] })),
        ("lspServers", json!({ "zls": { "command": "zls" } })),
        ("permissions", json!({ "deny": ["Bash(rm:*)"] })),
        ("verification", json!({ "allow": ["Bash(make)"] })),
        ("scripts", json!({ "network": false })),
        ("sandbox", json!({ "network": false })),
        ("features", json!({ "git-offer": false })),
        (domain::trust::LEGACY_OPT_IN, json!(true)),
    ]
}

#[test]
fn every_key_the_schema_publishes_changes_what_the_loader_reads() {
    let published = crate::config_schema::settings();
    let mut keys: Vec<&str> = published["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .map(String::as_str)
        .filter(|key| *key != "$schema")
        .collect();
    keys.sort_unstable();
    let samples = one_value_per_published_key();
    let mut sampled: Vec<&str> = samples.iter().map(|(key, _)| *key).collect();
    sampled.sort_unstable();
    assert_eq!(keys, sampled, "each published key has a sample here");

    let (home, repo) = folders();
    let unset = format!("{:?}", load(repo.path(), Some(home.path()), &[]).expect("settings"));
    for (key, value) in samples {
        write(home.path(), "settings.json", &json!({ key: value }));
        let read = load(repo.path(), Some(home.path()), &[]).expect("settings");
        assert!(!read.notices.iter().any(|notice| notice.contains(key) && notice.contains("ignored")), "{key}");
        assert_ne!(format!("{read:?}"), unset, "{key} is published but the loader does not read it");
    }
}
