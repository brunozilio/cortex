#[test]
#[cfg(unix)]
fn a_stdio_mcp_server_that_stays_silent_after_a_cancelled_call_is_stopped_before_it_can_write() {
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
*'"method":"tools/call"'*) touch '{}'; sleep 6; touch '{}'; printf '%s\n' '{{"id":3,"result":{{"content":[{{"type":"text","text":"done"}}]}}}}';;
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
        sandbox: Default::default(),
    };
    let http: Arc<dyn Http> = Arc::new(NetHttp);
    let server = Arc::new(McpServer::open(&spec, &http).expect("valid test fixture"));
    let tool = server.tools().expect("valid test fixture").remove(0);
    let workspace = Arc::new(Workspace::new(dir.path()));
    let worker = workspace.clone();
    let call = std::thread::spawn(move || tool.run(&json!({}), &worker));
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    while !started.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(started.exists());
    workspace.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let cancelled = std::time::Instant::now();
    assert!(call.join().expect("valid test fixture").is_error);
    assert!(cancelled.elapsed() < Duration::from_secs(2), "the cancelled call waited for the server");
    std::thread::sleep(Duration::from_millis(6500));
    assert!(!leaked.exists(), "MCP side effect occurred after cancellation");
}
#[test]
fn planning_mode_rejects_builtin_mutations() {
    use agent_engine::mcp::remote_effects;
    use domain::permission::{Bounds, Call, NothingAround, PolicyContext, PolicyEngine, Remembered, Verdict};
    use domain::permission_rules::PermissionRules;
    use domain::{PermissionMode, memory};
    let (rules, remembered, bounds) = (PermissionRules::default(), Remembered::default(), Bounds::default());
    let planning = PolicyContext {
        mode: PermissionMode::Plan,
        ceiling: None,
        rules: &rules,
        remembered: &remembered,
        bounds: &bounds,
        cwd: "/repo",
        home: None,
        sandboxed: false,
        sandbox_missing: None,
        no_prompts: false,
        surroundings: &NothingAround,
    };
    let decide = |server: &str, tool: &str| {
        let effects = remote_effects(server, tool);
        PolicyEngine::decide(&Call { tool, subject: "", effects: &effects }, &planning)
    };
    for (server, tool) in [("linear", "create_issue"), (memory::MEMORY_SERVER, "memory_forget")] {
        assert!(decide(server, tool).denies(), "{tool}");
    }
    assert_eq!(decide(memory::MEMORY_SERVER, memory::MEMORY_SEARCH_TOOL), Verdict::Allow);
}
