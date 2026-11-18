use super::load_trusted as load;
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
fn managed_settings_in_the_home_have_the_last_word_over_the_project() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let (user, project) = (home.path().join(brand::DIR), repo.path().join(brand::DIR));
    std::fs::create_dir_all(&user).expect("user");
    std::fs::create_dir_all(&project).expect("project");
    std::fs::write(project.join("settings.local.json"), r#"{"features":{"insights":true}}"#).expect("project");
    std::fs::write(user.join(MANAGED), r#"{"features":{"insights":false}}"#).expect("managed");
    let settings = load(repo.path(), Some(home.path()), &[]).expect("settings");
    assert!(!settings.features.on("insights"));
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
    let skipped = load(repo.path(), None, &[]).expect("a project's file that does not read is skipped");
    assert_eq!(skipped.judge, None);
    assert!(skipped.notices.iter().any(|notice| notice.contains("must be a model name")), "{:?}", skipped.notices);
    std::fs::write(root.join("settings.json"), r#"{"fallbackModel":["ok","two words"]}"#).expect("write");
    assert!(load(repo.path(), None, &[]).expect("skipped").fallbacks.is_empty());

    let home = tempfile::tempdir().expect("home");
    std::fs::create_dir_all(home.path().join(brand::DIR)).expect("dir");
    std::fs::write(home.path().join(brand::DIR).join("settings.json"), r#"{"judgeModel":"two words"}"#).expect("write");
    assert!(load(repo.path(), Some(home.path()), &[]).is_err(), "the owner's own file still stops the run");
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
    let settings = load(repo.path(), Some(home.path()), &[]).expect("a wrong list is left out, not fatal");
    assert!(settings.notices.iter().any(|notice| notice.contains("verification.allow: expected array")));
    let allowed: Vec<String> = settings.judge_allow.listed().into_iter().map(|(_, rule)| rule).collect();
    assert_eq!(allowed, ["Bash(make lint)"]);
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
    let groups = &settings.hooks.0[&crate::hooks::HookEvent::PreToolUse];
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].matcher, "Bash");
    assert_eq!(groups[0].hooks[0].command, format!("{}/check.sh", good.path().display()));
    assert_eq!(settings.hooks.0.len(), 1, "nothing of the broken plugin is kept");
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
    let scripts = load(repo.path(), Some(home.path()), &[]).expect("load").scripts.owner;
    assert!(!scripts.processes, "the repository cannot turn back on what the owner turned off");
    assert!(!scripts.network && scripts.files);

    write(repo.path().join(brand::DIR), "settings.local.json", serde_json::json!({ "processes": true }));
    let scripts = load(repo.path(), Some(home.path()), &[]).expect("load").scripts.owner;
    assert!(scripts.processes, "the owner's local file may");
}

#[test]
fn a_project_s_scripts_get_nothing_until_the_owner_s_settings_turn_it_on() {
    use crate::tools::script_tool::ScriptAccess;
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let write = |root: std::path::PathBuf, scripts: serde_json::Value| {
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join("settings.json"), serde_json::json!({ "scripts": scripts }).to_string())
            .expect("write");
    };
    let scripts = load(repo.path(), Some(home.path()), &[]).expect("load").scripts;
    assert_eq!(scripts.project, ScriptAccess::none());
    assert_eq!(scripts.owner, ScriptAccess::default(), "the user's own scripts keep everything");

    let all = serde_json::json!({ "files": true, "processes": true, "network": true });
    write(repo.path().join(brand::DIR), all);
    let scripts = load(repo.path(), Some(home.path()), &[]).expect("load").scripts;
    assert_eq!(scripts.project, ScriptAccess::none(), "the repository cannot grant itself access");

    std::fs::remove_file(repo.path().join(brand::DIR).join("settings.json")).expect("remove");
    write(home.path().join(brand::DIR), serde_json::json!({ "files": true }));
    let scripts = load(repo.path(), Some(home.path()), &[]).expect("load").scripts;
    assert_eq!(scripts.project, ScriptAccess { files: true, processes: false, network: false });
}

#[test]
fn only_the_owner_turns_the_sandbox_off_or_widens_it() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let write = |root: std::path::PathBuf, settings: serde_json::Value| {
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join("settings.json"), settings.to_string()).expect("write");
    };
    let checked_in = serde_json::json!({
        "features": { "experimental-sandbox": false },
        "sandbox": { "allowWrite": ["/etc"], "network": false }
    });
    write(repo.path().join(brand::DIR), checked_in);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert!(settings.features.on(domain::features::SANDBOX), "a repository cannot turn the sandbox off");
    assert!(settings.sandbox.allow_write.is_empty() && !settings.sandbox.network);

    let owners = serde_json::json!({
        "features": { "experimental-sandbox": false },
        "sandbox": { "allowWrite": ["~/.cargo"] }
    });
    write(home.path().join(brand::DIR), owners);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert!(!settings.features.on(domain::features::SANDBOX));
    assert_eq!(settings.sandbox.allow_write, ["~/.cargo"]);
}

#[test]
fn only_the_owner_stops_requiring_the_sandbox() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let write = |root: std::path::PathBuf, settings: serde_json::Value| {
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join("settings.json"), settings.to_string()).expect("write");
    };
    let lifted = serde_json::json!({ "sandbox": { "required": false } });
    let required = load(repo.path(), Some(home.path()), &[]).expect("load").sandbox.required;
    assert_eq!(required, domain::sandbox::REQUIRED_BY_DEFAULT);
    write(repo.path().join(brand::DIR), lifted.clone());
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert_eq!(settings.sandbox.required, domain::sandbox::REQUIRED_BY_DEFAULT, "a repository cannot lift it");
    std::fs::remove_file(repo.path().join(brand::DIR).join("settings.json")).expect("remove");
    write(home.path().join(brand::DIR), lifted);
    assert!(!load(repo.path(), Some(home.path()), &[]).expect("load").sandbox.required, "the owner can");
}

#[test]
fn teammate_mode_says_where_teammates_start_and_an_unknown_one_is_named_and_left_out() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let user = home.path().join(brand::DIR);
    std::fs::create_dir_all(&user).expect("user");
    assert_eq!(
        load(repo.path(), Some(home.path()), &[]).expect("settings").teammate_mode,
        domain::team::TeammateMode::Process
    );
    std::fs::write(user.join("settings.json"), r#"{"teammateMode":"auto"}"#).expect("write");
    assert_eq!(
        load(repo.path(), Some(home.path()), &[]).expect("settings").teammate_mode,
        domain::team::TeammateMode::Auto
    );
    std::fs::write(user.join("settings.json"), r#"{"teammateMode":"kitty"}"#).expect("write");
    let refused = load(repo.path(), Some(home.path()), &[]).expect("settings");
    assert_eq!(refused.teammate_mode, domain::team::TeammateMode::Process, "an unknown mode is not used");
    assert!(refused.notices.iter().any(|notice| notice.contains("teammateMode")), "{:?}", refused.notices);
}
