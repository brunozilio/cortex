//! Hooks around a custom agent's work: command, prompt, agent and HTTP
//! handlers, their decisions, rewritten input and time limits.

use std::path::Path;

use serde_json::json;
use superfik_core::PermissionMode;
use superfik_native::{model::scripted::ScriptedModel, tools::Workspace};

use crate::{call, delegated, done, run};

#[cfg(unix)]
#[test]
fn pre_hook_blocks_mutations_and_post_hook_receives_actual_result() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    workspace.worker = Some(Path::new(env!("CARGO_BIN_EXE_superfik-native-worker")).into());
    let model = ScriptedModel::new([
        delegated(),
        ScriptedModel::reply(vec![call("bad", "Bash", json!({"command":"echo bad > forbidden"}))]),
        done("Blocked"),
        done("Reviewed"),
    ]);
    run(
        json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"cat > hook-input.json; echo blocked >&2; exit 2"}]}]}}),
        &model,
        &workspace,
        PermissionMode::Bypass,
    );
    assert!(!dir.path().join("forbidden").exists());
    let input: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("hook-input.json")).expect("input"))
            .expect("json");
    assert_eq!(input["tool_name"], "Bash");
    assert_eq!(input["hook_event_name"], "PreToolUse");
    assert!(format!("{:?}", model.asked()[2].messages).contains("blocked"));
    let hooks = superfik_native::hooks::Hooks::parse(
        &json!({"PostToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"cat > actual-result.json"}]}]}),
    )
    .expect("hooks");
    workspace.hooks = hooks;
    workspace.hooks.run("PostToolUse", "Read", json!({"tool_response":"real result"}), &workspace).expect("post hook");
    let result = std::fs::read_to_string(dir.path().join("actual-result.json")).expect("saved");
    assert!(result.contains("real result"));
}
#[test]
fn hook_timeout_kills_a_running_process_tree() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let handler = superfik_native::hooks::Handler {
        kind: "command".into(),
        command: "sleep 20".into(),
        timeout: 1,
        status_message: None,
        prompt: None,
        model: None,
        url: None,
        headers: Default::default(),
        allowed_env_vars: vec![],
    };
    let start = std::time::Instant::now();
    assert!(superfik_native::hooks::command(&handler, &json!({}), &workspace).is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(4));
}

#[test]
fn scoped_stop_hook_can_require_one_more_turn_without_affecting_the_parent() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let model =
        ScriptedModel::new([delegated(), done("First report"), done("Verified report"), done("Final response")]);
    run(
        json!({"hooks":{"Stop":[{"hooks":[{"type":"command","command":"input=$(cat); case \"$input\" in *'\"stop_hook_active\":false'*) echo '{\"decision\":\"block\",\"reason\":\"Verify once before finishing\"}';; *) echo '{}';; esac"}]}]}}),
        &model,
        &workspace,
        PermissionMode::Bypass,
    );
    assert_eq!(model.asked().len(), 4);
    assert!(model.asked()[2].messages.last().expect("hook feedback").text().contains("Verify once"));
}

#[test]
fn prompt_hook_uses_the_model_to_reject_an_action_before_execution() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let model = ScriptedModel::new([
        delegated(),
        ScriptedModel::reply(vec![call("bad", "Bash", json!({"command":"echo bad > forbidden"}))]),
        done(r#"{"ok":false,"reason":"Mutation rejected"}"#),
        done("Blocked"),
        done("Reviewed"),
    ]);
    run(
        json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"prompt","prompt":"Approve only read operations: $ARGUMENTS"}]}]}}),
        &model,
        &workspace,
        PermissionMode::Bypass,
    );
    assert!(!dir.path().join("forbidden").exists());
    let asked = model.asked();
    assert!(asked[2].tools.is_empty());
    assert!(asked[2].messages[0].text().contains("echo bad"));
    assert!(format!("{:?}", asked[3].messages).contains("Mutation rejected"));
}
#[test]
fn agent_hook_can_inspect_files_with_read_only_tools_before_allowing_the_action() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("policy.txt"), "Allow this check").expect("policy");
    let workspace = Workspace::new(dir.path());
    let model = ScriptedModel::new([
        delegated(),
        ScriptedModel::reply(vec![call("check", "Bash", json!({"command":"printf checked"}))]),
        ScriptedModel::reply(vec![call("policy", "Read", json!({"file_path":"policy.txt"}))]),
        done(r#"{"ok":true,"reason":"Policy checked"}"#),
        done("Checked"),
        done("Reviewed"),
    ]);
    run(
        json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"agent","prompt":"Check policy.txt before approving $ARGUMENTS"}]}]}}),
        &model,
        &workspace,
        PermissionMode::Bypass,
    );
    let asked = model.asked();
    assert!(asked[2].tools.iter().all(|tool| matches!(tool.name.as_str(), "Read" | "Glob" | "Grep")));
    assert!(format!("{:?}", asked[3].messages).contains("Allow this check"));
}
#[test]
fn http_hook_receives_json_and_returns_a_blocking_decision_through_the_worker() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
    let addr = listener.local_addr().expect("address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).expect("timeout");
        let mut bytes = Vec::new();
        let mut buffer = [0; 1024];
        loop {
            let count = stream.read(&mut buffer).expect("read");
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                let len = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:").map(|s| s.trim().parse::<usize>().expect("length"))
                    })
                    .expect("length header");
                if bytes.len() >= end + 4 + len {
                    assert!(headers.contains("x-fixture: hook-value"));
                    let value: serde_json::Value =
                        serde_json::from_slice(&bytes[end + 4..end + 4 + len]).expect("hook JSON");
                    assert_eq!(value["tool_name"], "Bash");
                    break;
                }
            }
        }
        let body =
            r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"HTTP policy rejected"}}"#;
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).expect("reply");
    });
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    workspace.worker = Some(Path::new(env!("CARGO_BIN_EXE_superfik-native-worker")).into());
    let hooks=superfik_native::hooks::Hooks::parse(&json!({"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"http","url":format!("http://{addr}"),"headers":{"X-Fixture":"hook-value"},"timeout":5}]}]})).expect("hook");
    let decision = hooks.run("PreToolUse", "Bash", json!({"tool_name":"Bash"}), &workspace).expect("HTTP hook");
    assert_eq!(decision.blocked.as_deref(), Some("HTTP policy rejected"));
    server.join().expect("server");
}
#[test]
fn hook_allow_respects_parent_plan_mode() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let model = ScriptedModel::new([
        delegated(),
        ScriptedModel::reply(vec![call("bad", "Bash", json!({"command":"echo bad > forbidden"}))]),
        done("Denied"),
        done("Reviewed"),
    ]);
    run(
        json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"printf '%s' '{\"hookSpecificOutput\":{\"permissionDecision\":\"allow\"}}'"}]}]}}),
        &model,
        &workspace,
        PermissionMode::Plan,
    );
    assert!(!dir.path().join("forbidden").exists());
    assert!(format!("{:?}", model.asked()[2].messages).contains("only read and search"));
}
#[test]
fn pre_hook_replaces_the_actual_tool_arguments() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let model = ScriptedModel::new([
        delegated(),
        ScriptedModel::reply(vec![call("rewrite", "Bash", json!({"command":"echo bad > forbidden"}))]),
        done("Safe"),
        done("Reviewed"),
    ]);
    run(
        json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"printf '%s' '{\"hookSpecificOutput\":{\"updatedInput\":{\"command\":\"printf safe\"}}}'"}]}]}}),
        &model,
        &workspace,
        PermissionMode::Bypass,
    );
    assert!(!dir.path().join("forbidden").exists());
    assert!(format!("{:?}", model.asked()[2].messages).contains("safe"));
}

#[test]
fn completed_hook_shell_does_not_wait_for_a_pipe_inheriting_background_child() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let handler: superfik_native::hooks::Handler =
        serde_json::from_value(json!({"type":"command","command":"sleep 20 & exit 0","timeout":1})).expect("handler");
    let start = std::time::Instant::now();
    let result = superfik_native::hooks::command(&handler, &json!({}), &workspace).expect("completed shell");
    assert_eq!(result.code, Some(0));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
