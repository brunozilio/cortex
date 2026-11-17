use crate::{Sandbox, command};

fn kept(sandbox: &Sandbox) -> serde_json::Value {
    let text = std::fs::read_to_string(sandbox.user_dir().join("mcp.json")).expect("mcp.json written");
    serde_json::from_str(&text).expect("mcp.json is JSON")
}

#[test]
fn with_no_servers_the_list_says_how_to_add_one() {
    let sandbox = Sandbox::fresh();
    let hint = format!("No MCP servers. Add one with `{} mcp add <name> <url|command>`", command());
    sandbox.run(&["mcp"]).succeeded().quiet_on_stderr().said(&hint);
    sandbox.run(&["mcp", "list"]).succeeded().quiet_on_stderr().said(&hint);
}

#[test]
fn a_command_server_is_added_listed_and_removed() {
    let sandbox = Sandbox::fresh();
    let added = sandbox.run(&["mcp", "add", "-e", "TOKEN=${TOKEN}", "echo", "node", "--", "server.js", "--flag"]);
    added.succeeded().quiet_on_stderr();
    assert_eq!(added.stdout, "Added echo (user, ready): node server.js --flag\n");
    assert_eq!(
        kept(&sandbox)["mcpServers"]["echo"],
        serde_json::json!({"command": "node", "args": ["server.js", "--flag"], "env": {"TOKEN": "${TOKEN}"}})
    );

    let listed = sandbox.run(&["mcp", "list"]);
    listed.succeeded();
    assert!(listed.stdout.lines().any(|line| line.starts_with("echo") && line.contains("user")), "{listed:?}");

    sandbox.run(&["mcp", "remove", "echo"]).succeeded().said("No MCP servers.");
    assert_eq!(kept(&sandbox)["mcpServers"], serde_json::json!({}));
}

#[test]
fn an_http_server_keeps_its_headers_and_sse_transport() {
    let sandbox = Sandbox::fresh();
    let ran = sandbox.run(&[
        "mcp",
        "add",
        "--transport",
        "sse",
        "-H",
        "Authorization: Bearer ${TOKEN}",
        "docs",
        "https://mcp.example.invalid/sse",
    ]);
    ran.succeeded().said("Added docs (user, ready): https://mcp.example.invalid/sse");
    let server = &kept(&sandbox)["mcpServers"]["docs"];
    assert_eq!(server["type"], "sse");
    assert_eq!(server["headers"]["Authorization"], "Bearer ${TOKEN}");
}

#[test]
fn a_secret_written_into_a_header_is_refused_and_nothing_is_kept() {
    let sandbox = Sandbox::fresh();
    let ran = sandbox.run(&["mcp", "add", "-H", "Authorization: Bearer sk-live", "web", "https://e.invalid/mcp"]);
    ran.failed_with(1).quiet_on_stdout().complained("`headers.Authorization` writes a secret into the file");
    sandbox.run(&["mcp", "list"]).said("No MCP servers.");
}

#[test]
fn removing_a_server_that_is_not_there_fails_naming_it() {
    let ran = Sandbox::fresh().run(&["mcp", "remove", "ghost"]);
    ran.failed_with(1).quiet_on_stdout();
    assert_eq!(ran.stderr.trim_end(), format!("{}: `ghost` is not one of your MCP servers", command()));
}

#[test]
fn signing_out_of_a_server_that_is_not_there_fails_naming_it() {
    let ran = Sandbox::fresh().run(&["mcp", "logout", "ghost"]);
    ran.failed_with(1).quiet_on_stdout().complained("No MCP server `ghost` here");
}

#[test]
fn an_env_pair_without_an_equals_sign_is_a_usage_error() {
    let ran = Sandbox::fresh().run(&["mcp", "add", "-e", "NOEQ", "x", "cmd"]);
    ran.failed_with(2).quiet_on_stdout().complained("`NOEQ` needs a `=`");
}

#[test]
fn add_needs_a_name_and_a_target() {
    let ran = Sandbox::fresh().run(&["mcp", "add", "lonely"]);
    ran.failed_with(2).quiet_on_stdout().complained("<TARGET>");
}

#[test]
fn a_user_config_that_is_not_json_is_named_when_changing_it() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.user_dir().join("mcp.json");
    sandbox.write(&file, "{broken");
    for args in [["mcp", "add", "x", "cmd"].as_slice(), &["mcp", "remove", "x"]] {
        let ran = sandbox.run(args);
        ran.failed_with(1).quiet_on_stdout().complained(&format!("{}: {}: ", command(), file.display()));
    }
    assert_eq!(std::fs::read_to_string(&file).expect("kept"), "{broken", "the broken file is not overwritten");
}

#[test]
fn a_project_settings_file_that_is_not_json_does_not_stop_the_list() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.project_dir().join("settings.json");
    sandbox.write(&file, "{broken");
    let ran = sandbox.run(&["mcp", "list"]);
    ran.succeeded();
    assert!(!ran.stderr.contains("Could not read the MCP servers"), "{ran:?}");
}

fn project_servers(sandbox: &Sandbox, servers: &str) {
    sandbox.write(&sandbox.project_dir().join("mcp.json"), &format!(r#"{{"mcpServers": {servers}}}"#));
}

#[test]
fn a_user_config_that_is_not_json_fails_the_list_naming_it_and_its_error() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.user_dir().join("mcp.json");
    sandbox.write(&file, "{broken");
    let ran = sandbox.run(&["mcp", "list"]);
    ran.failed_with(1).complained(&file.display().to_string()).complained("key must be a string");
    assert!(!ran.stdout.contains("No MCP servers"), "{ran:?}");
}

#[test]
fn a_project_config_that_is_not_json_is_a_warning_on_the_list() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.project_dir().join("mcp.json");
    sandbox.write(&file, "{broken");
    let ran = sandbox.run(&["mcp", "list"]);
    ran.succeeded().complained(&format!("warning: {}: key must be a string", file.display()));
}

#[test]
fn approving_or_denying_a_server_that_is_not_there_fails_naming_it_and_the_known_ones() {
    let sandbox = Sandbox::fresh();
    project_servers(&sandbox, r#"{"jira": {"command": "jira-mcp"}}"#);
    for verb in ["approve", "deny"] {
        let ran = sandbox.run(&["mcp", verb, "ghost"]);
        ran.failed_with(1).quiet_on_stdout().complained("No MCP server `ghost` here; known: jira");
    }
    assert!(!sandbox.project_dir().join("settings.local.json").exists(), "no approval was written");
    sandbox.run(&["mcp", "approve", "jira"]).succeeded();
}

#[test]
fn adding_a_name_that_is_taken_is_refused_unless_forced() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["mcp", "add", "echo", "node"]).succeeded();
    let ran = sandbox.run(&["mcp", "add", "echo", "deno"]);
    ran.failed_with(1).quiet_on_stdout().complained("`echo` is already one of your MCP servers (node)");
    ran.complained("--force");
    assert_eq!(kept(&sandbox)["mcpServers"]["echo"]["command"], "node", "the first one is kept");
    sandbox.run(&["mcp", "add", "--force", "echo", "deno"]).succeeded().said("Added echo (user, ready): deno");
    assert_eq!(kept(&sandbox)["mcpServers"]["echo"]["command"], "deno");
}

#[test]
fn an_untrusted_project_s_servers_are_listed_as_skipped_and_counted_by_doctor() {
    let sandbox = Sandbox::fresh();
    project_servers(&sandbox, r#"{"jira": {"command": "jira-mcp"}}"#);
    let ran = sandbox.run(&["mcp", "list"]);
    ran.succeeded();
    let jira = ran.stdout.lines().find(|line| line.starts_with("jira")).unwrap_or_else(|| panic!("{ran:?}"));
    assert!(jira.contains("project") && jira.contains("skipped: folder not trusted"), "{ran:?}");
    let doctor = sandbox.run(&["doctor"]);
    let trust = doctor.stdout.lines().find(|line| line.contains("trust")).unwrap_or_default();
    assert!(trust.contains("MCP servers (1)"), "{doctor:?}");
}

#[test]
fn options_after_the_command_are_ours_and_the_server_s_own_follow_a_double_dash() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["mcp", "add", "echo", "node", "-e", "K=V"]).succeeded();
    assert_eq!(kept(&sandbox)["mcpServers"]["echo"], serde_json::json!({"command": "node", "env": {"K": "V"}}));

    let ran = sandbox.run(&["mcp", "add", "other", "node", "server.js", "--flag"]);
    ran.failed_with(2).quiet_on_stdout().complained("'-- --flag'");

    sandbox.run(&["mcp", "add", "npx", "--", "npx", "-y", "pkg"]).succeeded();
    assert_eq!(kept(&sandbox)["mcpServers"]["npx"], serde_json::json!({"command": "npx", "args": ["-y", "pkg"]}));

    let help = sandbox.run(&["mcp", "add", "--help"]);
    help.succeeded().said(" -- ");
}

#[test]
fn get_prints_one_server_s_entry_with_its_secrets_hidden() {
    let sandbox = Sandbox::fresh();
    sandbox.write(
        &sandbox.user_dir().join("mcp.json"),
        r#"{"mcpServers": {"docs": {"type": "http", "url": "https://e.invalid/mcp",
            "headers": {"Authorization": "Bearer sk-live-abcdef0123456789", "X-Toolsets": "repos"}}}}"#,
    );
    let ran = sandbox.run(&["mcp", "get", "docs"]);
    ran.succeeded().quiet_on_stderr().said("docs").said("user").said("https://e.invalid/mcp").said("repos");
    assert!(!ran.stdout.contains("sk-live"), "{ran:?}");
    ran.said(&sandbox.user_dir().join("mcp.json").display().to_string());

    let ghost = sandbox.run(&["mcp", "get", "ghost"]);
    ghost.failed_with(1).quiet_on_stdout().complained("No MCP server `ghost` here; known: docs");
}
