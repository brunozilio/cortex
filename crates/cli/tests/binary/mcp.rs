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
    let added = sandbox.run(&["mcp", "add", "-e", "TOKEN=${TOKEN}", "echo", "node", "server.js", "--flag"]);
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
fn a_project_settings_file_that_is_not_json_fails_the_list_naming_it() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.project_dir().join("settings.json");
    sandbox.write(&file, "{broken");
    let ran = sandbox.run(&["mcp", "list"]);
    ran.failed_with(1)
        .quiet_on_stdout()
        .complained("Could not read the MCP servers")
        .complained(&file.display().to_string());
}
