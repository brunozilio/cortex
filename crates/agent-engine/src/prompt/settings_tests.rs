use super::*;

#[test]
fn project_overrides_user_and_claude_settings_are_not_read() {
    let home = tempfile::tempdir().expect("valid test fixture");
    let repo = tempfile::tempdir().expect("valid test fixture");
    for (root, style) in [
        (home.path().join(brand::DIR), "Explanatory"),
        (repo.path().join(brand::DIR), "Concise"),
        (repo.path().join(".claude"), "Learning"),
    ] {
        std::fs::create_dir_all(&root).expect("valid test fixture");
        std::fs::write(root.join("settings.json"), serde_json::json!({"outputStyle":style}).to_string())
            .expect("valid test fixture");
    }
    let config = load(repo.path(), Some(home.path()), &[]).expect("valid test fixture");
    assert_eq!(config.style.expect("valid test fixture").name, "Concise");
}

#[test]
fn custom_style_controls_coding_instructions_and_rejects_paths() {
    let repo = tempfile::tempdir().expect("valid test fixture");
    let root = repo.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("output-styles")).expect("valid test fixture");
    std::fs::write(root.join("settings.json"), r#"{"outputStyle":"review"}"#).expect("valid test fixture");
    std::fs::write(
        root.join("output-styles/review.md"),
        "---\nkeep-coding-instructions: false\n---\nReturn review findings.",
    )
    .expect("valid test fixture");
    let style = load(repo.path(), None, &[]).expect("valid test fixture").style.expect("valid test fixture");
    assert!(!style.keep_coding);
    assert_eq!(style.instructions, "Return review findings.");
    std::fs::write(root.join("settings.json"), r#"{"outputStyle":"../private"}"#).expect("valid test fixture");
    assert!(load(repo.path(), None, &[]).is_err());
}

#[test]
fn judge_and_fallback_models_are_model_names() {
    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path().join(brand::DIR);
    std::fs::create_dir_all(&root).expect("dir");
    std::fs::write(
        root.join("settings.json"),
        r#"{"judgeModel":"claude-haiku-5-5","fallbackModel":"claude-sonnet-5-5"}"#,
    )
    .expect("write");
    let settings = load(repo.path(), None, &[]).expect("settings");
    assert_eq!(settings.judge.as_deref(), Some("claude-haiku-5-5"));
    assert_eq!(settings.fallbacks, ["claude-sonnet-5-5"]);
    std::fs::write(root.join("settings.json"), r#"{"fallbackModel":["chatgpt:a","kiro:b"]}"#).expect("write");
    assert_eq!(load(repo.path(), None, &[]).expect("settings").fallbacks, ["chatgpt:a", "kiro:b"]);
    std::fs::write(root.join("settings.json"), r#"{"judgeModel":"two words"}"#).expect("write");
    assert!(load(repo.path(), None, &[]).is_err());
    std::fs::write(root.join("settings.json"), r#"{"fallbackModel":["ok","two words"]}"#).expect("write");
    assert!(load(repo.path(), None, &[]).is_err());
}

#[test]
fn only_the_owner_lets_the_judge_run_commands_without_asking() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let write = |path: std::path::PathBuf, text: &str| {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("write");
    };
    write(
        repo.path().join(concat!(brand::dir!(), "/settings.json")),
        r#"{"verification": {"allow": ["Bash(curl:*)"]}}"#,
    );
    write(
        repo.path().join(concat!(brand::dir!(), "/settings.local.json")),
        r#"{"verification": {"allow": ["Bash(npm run test:*)"]}}"#,
    );
    write(
        home.path().join(concat!(brand::dir!(), "/settings.json")),
        r#"{"verification": {"allow": ["Bash(make lint)"]}}"#,
    );
    let allowed: Vec<String> = load(repo.path(), Some(home.path()), &[])
        .expect("settings")
        .judge_allow
        .listed()
        .into_iter()
        .map(|(_, rule)| rule)
        .collect();
    assert_eq!(allowed, ["Bash(make lint)", "Bash(npm run test:*)"], "a checked-in file cannot allow commands");
    write(repo.path().join(concat!(brand::dir!(), "/settings.local.json")), r#"{"verification": {"allow": "x"}}"#);
    assert!(load(repo.path(), Some(home.path()), &[]).is_err_and(|error| error.contains("verification.allow")));
}

#[test]
fn only_the_owner_approves_a_project_s_mcp_servers() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let write = |path: std::path::PathBuf, text: &str| {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("write");
    };
    write(repo.path().join(concat!(brand::dir!(), "/settings.json")), r#"{"enableAllProjectMcpServers": true}"#);
    write(repo.path().join(concat!(brand::dir!(), "/settings.local.json")), r#"{"enabledMcpServers": ["jira"]}"#);
    write(home.path().join(concat!(brand::dir!(), "/settings.json")), r#"{"disabledMcpServers": ["sonar"]}"#);
    let approval = load(repo.path(), Some(home.path()), &[]).expect("settings").mcp_approval;
    assert!(!approval.all, "a checked-in setting cannot approve its own servers");
    assert_eq!(approval.enabled, ["jira"]);
    assert_eq!(approval.disabled, ["sonar"]);
}

#[test]
fn an_installed_plugin_adds_its_hooks_and_a_broken_one_is_left_out() {
    let repo = tempfile::tempdir().expect("repo");
    let good = tempfile::tempdir().expect("plugin");
    let broken = tempfile::tempdir().expect("plugin");
    std::fs::create_dir_all(good.path().join("hooks")).expect("hooks dir");
    std::fs::write(
        good.path().join("hooks/hooks.json"),
        concat!(
            r#"{"description": "Checks", "hooks": {"PreToolUse": [{"matcher": "Bash",
            "hooks": [{"type": "command", "command": "${"#,
            brand::env_prefix!(),
            r#"_PLUGIN_ROOT}/check.sh"}]}]}}"#
        ),
    )
    .expect("hooks");
    std::fs::create_dir_all(broken.path().join("hooks")).expect("hooks dir");
    std::fs::write(broken.path().join("hooks/hooks.json"), r#"{"hooks": {"NoSuchEvent": []}}"#).expect("hooks");
    let plugins = [crate::plugins::Plugin::at(good.path()), crate::plugins::Plugin::at(broken.path())];
    let settings = load(repo.path(), None, &plugins).expect("a broken plugin does not stop the run");
    let groups = &settings.hooks.0["PreToolUse"];
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].matcher, "Bash");
    assert_eq!(groups[0].hooks[0].command, format!("{}/check.sh", good.path().display()));
    assert!(!settings.hooks.0.contains_key("NoSuchEvent"));
}

#[test]
fn a_checked_in_file_can_only_take_script_access_away() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let write = |root: std::path::PathBuf, name: &str, scripts: serde_json::Value| {
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join(name), serde_json::json!({ "scripts": scripts }).to_string()).expect("write");
    };
    write(home.path().join(brand::DIR), "settings.json", serde_json::json!({ "processes": false }));
    write(repo.path().join(brand::DIR), "settings.json", serde_json::json!({ "processes": true, "network": false }));
    let scripts = load(repo.path(), Some(home.path()), &[]).expect("load").scripts;
    assert!(!scripts.processes, "the repository cannot turn back on what the owner turned off");
    assert!(!scripts.network && scripts.files);

    write(repo.path().join(brand::DIR), "settings.local.json", serde_json::json!({ "processes": true }));
    let scripts = load(repo.path(), Some(home.path()), &[]).expect("load").scripts;
    assert!(scripts.processes, "the owner's local file may");
}
