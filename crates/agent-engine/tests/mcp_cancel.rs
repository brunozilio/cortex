#[test]
#[cfg(unix)]
fn cancelling_stdio_mcp_stops_its_process_tree_before_it_can_write() {
    use agent_engine::{
        mcp::{McpServer, ServerSpec},
        model::http::{Http, NetHttp},
        tools::{Tool, Workspace},
    };
    use serde_json::json;
    use std::{sync::Arc, time::Duration};
    let dir = tempfile::tempdir().expect("valid test fixture");
    let started = dir.path().join("started");
    let leaked = dir.path().join("leaked");
    let script = format!(
        r#"while IFS= read -r line; do
case "$line" in
*'"method":"initialize"'*) printf '%s\n' '{{"id":1,"result":{{}}}}';;
*'"method":"tools/list"'*) printf '%s\n' '{{"id":2,"result":{{"tools":[{{"name":"slow","inputSchema":{{"type":"object"}}}}]}}}}';;
*'"method":"tools/call"'*) touch '{}'; sleep 1; touch '{}'; printf '%s\n' '{{"id":3,"result":{{"content":[{{"type":"text","text":"done"}}]}}}}';;
esac
done"#,
        started.display(),
        leaked.display()
    );
    let spec = ServerSpec::Stdio {
        name: "fixture".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script],
        env: vec![],
    };
    let http: Arc<dyn Http> = Arc::new(NetHttp);
    let server = Arc::new(McpServer::open(&spec, &http).expect("valid test fixture"));
    let tool = server.tools().expect("valid test fixture").remove(0);
    let workspace = Arc::new(Workspace::new(dir.path()));
    let worker = workspace.clone();
    let call = std::thread::spawn(move || tool.run(&json!({}), &worker));
    for _ in 0..200 {
        if started.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(started.exists());
    workspace.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(call.join().expect("valid test fixture").is_error);
    std::thread::sleep(Duration::from_millis(1100));
    assert!(!leaked.exists(), "MCP side effect occurred after cancellation");
}
#[test]
fn planning_mode_rejects_builtin_mutations() {
    use agent_engine::{
        mcp::builtin_access,
        permission::{Remembered, Verdict, verdict},
    };
    use domain::{PermissionMode, memory};
    for (server, tool) in [("linear", "create_issue"), (memory::MEMORY_SERVER, "memory_forget")] {
        assert!(
            matches!(
                verdict(PermissionMode::Plan, builtin_access(server, tool), tool, "", &Remembered::default()),
                Verdict::Deny(_)
            ),
            "{tool}"
        );
    }
    assert_eq!(
        verdict(
            PermissionMode::Plan,
            builtin_access(memory::MEMORY_SERVER, memory::MEMORY_SEARCH_TOOL),
            "memory_search",
            "",
            &Remembered::default()
        ),
        Verdict::Allow
    );
}
