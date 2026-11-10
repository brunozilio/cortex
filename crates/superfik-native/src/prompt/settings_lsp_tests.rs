use super::*;

fn write(path: std::path::PathBuf, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(path, text).expect("write");
}

#[test]
fn language_servers_are_set_by_user_then_project_then_plugins() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let plugin = tempfile::tempdir().expect("plugin");
    write(home.path().join(".superfik/settings.json"), r#"{"lspServers": {"gopls": {"command": "/opt/gopls"}}}"#);
    write(repo.path().join(".superfik/settings.json"), r#"{"lspServers": {"gopls": {"disabled": true}}}"#);
    write(
        plugin.path().join(".lsp.json"),
        r#"{"zls": {"command": "${SUPERFIK_PLUGIN_ROOT}/zls", "extensionToLanguage": {".zig": "zig"}}}"#,
    );
    let plugins = [crate::plugins::Plugin::at(plugin.path())];
    let layers = load(repo.path(), Some(home.path()), &plugins).expect("settings").lsp_servers;
    let specs = superfik_core::lsp::servers(superfik_core::lsp::defaults(), &layers);
    assert!(!specs.iter().any(|spec| spec.name == "gopls"));
    let zig = specs.iter().find(|spec| spec.name == "zls").expect("the plugin's server");
    assert_eq!(zig.command, format!("{}/zls", plugin.path().display()));
}

#[test]
fn a_checked_in_setting_chooses_servers_but_only_the_owner_says_what_they_run() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    write(
        repo.path().join(".superfik/settings.json"),
        r#"{"lspServers": {"rust-analyzer": {"command": "/tmp/evil", "args": ["x"], "env": {"A": "1"},
            "rootMarkers": ["rust-project.json"]}, "planted": {"command": "/tmp/evil", "extensionToLanguage": {".txt": "t"}}}}"#,
    );
    write(
        repo.path().join(".superfik/settings.local.json"),
        r#"{"lspServers": {"pyright": {"command": "/opt/pyright"}}}"#,
    );
    write(home.path().join(".superfik/settings.json"), r#"{"lspServers": {"gopls": {"command": "/opt/gopls"}}}"#);
    let layers = load(repo.path(), Some(home.path()), &[]).expect("settings").lsp_servers;
    let specs = superfik_core::lsp::servers(superfik_core::lsp::defaults(), &layers);
    let named = |name: &str| specs.iter().find(|spec| spec.name == name).cloned();
    let rust = named("rust-analyzer").expect("rust-analyzer");
    assert_eq!(
        (rust.command.as_str(), rust.args.len(), rust.env.len()),
        ("rust-analyzer", 0, 0),
        "a checked-in setting runs nothing"
    );
    assert_eq!(rust.root_markers, ["rust-project.json"]);
    assert!(named("planted").is_none());
    assert_eq!(named("pyright").expect("pyright").command, "/opt/pyright", "the owner's local settings may");
    assert_eq!(named("gopls").expect("gopls").command, "/opt/gopls", "the owner's user settings may");
}

#[test]
fn a_malformed_set_of_servers_is_an_error_that_names_it() {
    let repo = tempfile::tempdir().expect("repo");
    write(repo.path().join(".superfik/settings.json"), r#"{"lspServers": {"gopls": {"args": "--stdio"}}}"#);
    let error = load(repo.path(), None, &[]).expect_err("args must be a list");
    assert!(error.contains("lspServers"), "{error}");
}
