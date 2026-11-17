use domain::{Extensions, ModelOption};

use super::*;
use crate::model::http::ScriptedHttp;
use crate::tools::{Effect, Workspace};

fn acts(tool: &dyn Tool) -> bool {
    tool.effects(&serde_json::Value::Null, &Workspace::new("/repo")).iter().any(Effect::acts)
}

fn tools(request: &PromptRequest, http: &Arc<dyn Http>) -> Vec<Box<dyn Tool>> {
    tools_stopped(request, &McpSources::default(), http, &platform::Stop::new())
}

fn listed(id: u64) -> String {
    json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "tools": [{
            "name": "search",
            "description": "Searches the docs.",
            "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } },
        }] },
    })
    .to_string()
}

fn docs_request() -> (tempfile::TempDir, PromptRequest) {
    let dir = tempfile::tempdir().expect("dir");
    let config = dir.path().join("mcp.json");
    let server = r#"{"mcpServers": {"docs": {"type": "http", "url": "http://127.0.0.1:7/mcp", "headers": {"authorization": "Bearer t-1"}}}}"#;
    std::fs::write(&config, server).expect("config");
    let mut request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
    request.host.extensions = Some(Extensions { mcp_config: Some(config), ..Extensions::default() });
    (dir, request)
}

#[test]
fn an_http_server_s_tools_are_named_for_it_and_called_with_its_headers() {
    let initialized = json!({ "jsonrpc": "2.0", "id": 1, "result": { "capabilities": { "tools": {} } } });
    let called =
        json!({ "jsonrpc": "2.0", "id": 3, "result": { "content": [{ "type": "text", "text": "[4] Uses wgpu" }] } });
    let http = Arc::new(ScriptedHttp::new([
        (200, initialized.to_string()),
        (202, String::new()),
        (200, listed(2)),
        (200, called.to_string()),
    ]));
    let shared: Arc<dyn Http> = http.clone();
    let (_dir, request) = docs_request();
    let tools = tools(&request, &shared);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].spec().name, "mcp__docs__search");
    assert!(acts(&*tools[0]), "another's tools ask first");
    assert_eq!(tools[0].spec().schema["properties"]["query"]["type"], "string");
    let input = json!({ "query": "renderer" });
    assert_eq!(tools[0].summary(&input), "renderer");
    assert_eq!(tools[0].run(&input, &Workspace::new("/repo")), Outcome::ok("[4] Uses wgpu"));
    let sent = http.sent();
    assert!(sent.iter().all(|s| s.url == "http://127.0.0.1:7/mcp"));
    assert!(sent.iter().all(|s| s.headers.contains(&("authorization".to_owned(), "Bearer t-1".to_owned()))));
    let call: Value = serde_json::from_str(&sent[3].body).expect("json");
    assert_eq!((call["method"].as_str(), call["params"]["name"].as_str()), (Some("tools/call"), Some("search")));
}

#[test]
fn a_failed_call_and_an_unreachable_server_say_so() {
    let failed = json!({ "jsonrpc": "2.0", "id": 1, "result": { "content": [{ "type": "text", "text": "no" }], "isError": true } });
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([(200, failed.to_string()), (401, String::new())]));
    let (_dir, request) = docs_request();
    let spec = servers(request.folder(), &super::McpSources::default()).remove(0);
    let server = McpServer::open(&spec, &http).expect("open");
    let stop = platform::Stop::new();
    assert_eq!(server.call("search", &Value::Null, &stop), Outcome::error("no"));
    assert!(server.call("search", &Value::Null, &stop).content.contains("HTTP 401"));
    let none: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    assert!(tools(&request, &none).is_empty(), "the run goes on without it");
}

#[test]
fn an_answer_sent_as_a_stream_is_read_too() {
    let streamed = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n";
    assert_eq!(parse(streamed), Some(json!({ "jsonrpc": "2.0", "id": 1, "result": {} })));
}

#[cfg(unix)]
#[test]
fn a_process_server_s_tools_are_listed_and_called() {
    let script = r#"
echo 'starting up'
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  [ -z "$id" ] && continue
  case "$line" in
    *'"initialize"'*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}" ;;
    *'"tools/list"'*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"echo\",\"inputSchema\":{\"type\":\"object\"}}]}}" ;;
    *'"tools/call"'*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"hi $GREETING\"}]}}" ;;
  esac
done
"#;
    let spec = ServerSpec::Stdio {
        name: "echo-me".into(),
        command: "sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![("GREETING".into(), "there".into())],
        sandbox: Default::default(),
    };
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let server = Arc::new(McpServer::open(&spec, &http).expect("spawn"));
    let tools = server.tools().expect("tools");
    assert_eq!(tools[0].spec().name, "mcp__echo-me__echo");
    assert!(acts(&tools[0]), "a host's tool is asked for");
    assert_eq!(tools[0].run(&json!({}), &Workspace::new("/")), Outcome::ok("hi there"));
}

#[cfg(unix)]
#[test]
fn a_stdio_server_in_the_sandbox_writes_in_the_project_and_nowhere_else() {
    if !platform::sandbox::supported() {
        return;
    }
    let project = tempfile::tempdir().expect("project");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let (inside, outside) = (project.path().join("kept.txt"), elsewhere.path().join("made.txt"));
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  [ -z "$id" ] && continue
  case "$line" in
    *'"initialize"'*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}" ;;
    *'"tools/list"'*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"write\",\"inputSchema\":{\"type\":\"object\"}}]}}" ;;
    *'"tools/call"'*) echo in > "$0"; echo out > "$1" 2>/dev/null; echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"tried\"}]}}" ;;
  esac
done
"#;
    let confinement = platform::sandbox::Confinement {
        writable: vec![project.path().to_path_buf()],
        network: true,
        ..platform::sandbox::Confinement::default()
    };
    let spec = ServerSpec::Stdio {
        name: "writer".into(),
        command: "sh".into(),
        args: vec!["-c".into(), script.into(), inside.display().to_string(), outside.display().to_string()],
        env: vec![],
        sandbox: StdioSandbox::Confined(confinement),
    };
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let server = Arc::new(McpServer::open(&spec, &http).expect("spawn"));
    let tools = server.tools().expect("tools");
    assert_eq!(tools[0].run(&json!({}), &Workspace::new(project.path())), Outcome::ok("tried"));
    assert!(inside.exists(), "the project stays writable");
    assert!(!outside.exists(), "the sandbox kept the server's write out of other folders");
}

#[test]
fn a_stdio_server_the_required_sandbox_cannot_hold_does_not_start() {
    let spec = ServerSpec::Stdio {
        name: "jira".into(),
        command: "sh".into(),
        args: vec![],
        env: vec![],
        sandbox: StdioSandbox::Refused(domain::sandbox::refusal("this kernel has no Landlock")),
    };
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let error = McpServer::open(&spec, &http).expect_err("refused").to_string();
    assert!(error.contains("jira could not start") && error.contains("sandbox.required"), "{error}");
}

#[test]
fn a_command_that_does_not_exist_is_left_out() {
    let spec = ServerSpec::Stdio {
        name: "gone".into(),
        command: concat!(brand::command!(), "-no-such-tool").into(),
        args: vec![],
        env: vec![],
        sandbox: Default::default(),
    };
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let error = McpServer::open(&spec, &http).expect_err("missing").to_string();
    assert!(error.contains("gone could not start"), "{error}");
}

#[test]
fn lists_every_page_and_rejects_a_repeated_mcp_cursor() {
    let initialized = json!({"id":1,"result":{}}).to_string();
    let first = json!({"id":2,"result":{"tools":[{"name":"first"}],"nextCursor":"next"}}).to_string();
    let last = json!({"id":3,"result":{"tools":[{"name":"second"}]}}).to_string();
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([
        (200, initialized.clone()),
        (202, String::new()),
        (200, first.clone()),
        (200, last),
    ]));
    let spec = ServerSpec::Http { name: "pages".into(), url: "http://fixture".into(), headers: vec![] };
    let tools = Arc::new(McpServer::open(&spec, &http).expect("server")).tools().expect("all pages");
    assert_eq!(tools.iter().map(|t| t.spec().name).collect::<Vec<_>>(), ["mcp__pages__first", "mcp__pages__second"]);
    let repeated = json!({"id":3,"result":{"tools":[],"nextCursor":"next"}}).to_string();
    let http: Arc<dyn Http> =
        Arc::new(ScriptedHttp::new([(200, initialized), (202, String::new()), (200, first), (200, repeated)]));
    let error = Arc::new(McpServer::open(&spec, &http).expect("server")).tools().expect_err("repeated cursor");
    assert!(error.to_string().contains("repeated"));
}

#[test]
fn embedded_resource_links_and_structured_results_survive_tool_output() {
    let result = outcome(
        &json!({"content":[{"type":"resource","resource":{"uri":"fixture://resource","text":"ORANGE-RESOURCE"}},{"type":"resource_link","uri":"fixture://link","name":"Linked file"}],"structuredContent":{"summary":"ORANGE-STRUCTURED"}}),
    );
    assert!(!result.is_error);
    for expected in ["ORANGE-RESOURCE", "ORANGE-STRUCTURED", "fixture://link"] {
        assert!(result.content.contains(expected), "{expected}");
    }
    assert!(outcome(&json!({"content":[{"type":"audio","data":"not-supported"}]})).is_error);
}

#[test]
fn a_structured_result_reaches_the_model_on_one_line() {
    let result = outcome(&json!({"structuredContent":{"items":[{"id":1,"name":"a"},{"id":2,"name":"b"}]}}));
    assert_eq!(result.content, "Structured result:\n{\"items\":[{\"id\":1,\"name\":\"a\"},{\"id\":2,\"name\":\"b\"}]}");
}

struct Echo;

impl domain::mcp::McpEndpoint for Echo {
    fn handle(&self, message: &Value) -> Option<Value> {
        let id = message.get("id")?.clone();
        let result = match message["method"].as_str() {
            Some("tools/list") => json!({ "tools": [{ "name": "echo", "inputSchema": { "type": "object" } }] }),
            Some("tools/call") => {
                json!({ "content": [{ "type": "text", "text": message["params"]["arguments"]["say"].clone() }] })
            }
            _ => json!({}),
        };
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

#[test]
fn an_in_process_server_s_tools_are_called_without_http() {
    let mut request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
    request.host.credentials.mcp = vec![domain::mcp::LocalMcp::new("host", Arc::new(Echo))];
    let http = Arc::new(ScriptedHttp::new([]));
    let shared: Arc<dyn Http> = http.clone();
    let tools = tools(&request, &shared);
    assert_eq!(tools.iter().map(|t| t.spec().name).collect::<Vec<_>>(), ["mcp__host__echo"]);
    assert_eq!(tools[0].run(&json!({ "say": "hello" }), &Workspace::new("/repo")), Outcome::ok("hello"));
    assert!(http.sent().is_empty(), "nothing went over HTTP");
}

struct Memory;

impl domain::mcp::McpEndpoint for Memory {
    fn handle(&self, message: &Value) -> Option<Value> {
        let tools = || vec![json!({ "name": "memory_search", "inputSchema": { "type": "object" } })];
        domain::mcp::respond(message, concat!(brand::command!(), "_memory"), tools, |_, _| {
            Ok(domain::mcp::text_result("[4] Uses wgpu", false))
        })
    }
}

#[test]
fn brand_s_own_memory_search_runs_without_asking() {
    let mut request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
    let memory = domain::mcp::LocalMcp::new(domain::memory::MEMORY_SERVER, Arc::new(Memory));
    request.host.credentials.mcp = vec![memory];
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let tools = tools(&request, &http);
    assert_eq!(tools[0].spec().name, concat!("mcp__", brand::command!(), "_memory__memory_search"));
    assert!(!acts(&*tools[0]));
    let input = json!({ "query": "renderer" });
    assert_eq!(tools[0].summary(&input), "renderer");
    assert_eq!(tools[0].run(&input, &Workspace::new("/repo")), Outcome::ok("[4] Uses wgpu"));
}

struct Jira;

impl domain::mcp::McpEndpoint for Jira {
    fn handle(&self, message: &Value) -> Option<Value> {
        let tools = || {
            let object = json!({ "type": "object" });
            vec![
                json!({ "name": "search", "inputSchema": object, "annotations": { "readOnlyHint": true } }),
                json!({ "name": "create", "inputSchema": object, "annotations": { "readOnlyHint": false } }),
                json!({ "name": "edit", "inputSchema": object, "annotations": { "readOnlyHint": "true" } }),
                json!({ "name": "comment", "inputSchema": object }),
            ]
        };
        domain::mcp::respond(message, "jira", tools, |_, _| Ok(domain::mcp::text_result("ok", false)))
    }
}

#[test]
fn a_tool_its_server_marks_read_only_runs_without_asking_and_every_other_asks() {
    let mut request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
    request.host.credentials.mcp = vec![domain::mcp::LocalMcp::new("jira", Arc::new(Jira))];
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let asking: Vec<(String, bool)> = tools(&request, &http).iter().map(|t| (t.spec().name, acts(&**t))).collect();
    assert_eq!(
        asking,
        [
            ("mcp__jira__search".to_owned(), false),
            ("mcp__jira__create".to_owned(), true),
            ("mcp__jira__edit".to_owned(), true),
            ("mcp__jira__comment".to_owned(), true),
        ]
    );
}

struct SignedIn;

impl domain::credentials::McpTokenSource for SignedIn {
    fn token(&self, server_url: &str) -> Option<String> {
        (server_url == "https://mcp.test/mcp").then(|| "oauth-1".to_owned())
    }
}

#[test]
fn a_server_the_owner_signed_in_to_gets_their_token_and_one_refusing_says_how_to_sign_in() {
    let dir = tempfile::tempdir().expect("dir");
    let config = dir.path().join("mcp.json");
    let servers = r#"{"mcpServers": {"linear": {"type": "http", "url": "https://mcp.test/mcp"}}}"#;
    std::fs::write(&config, servers).expect("config");
    let mut request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
    request.host.extensions = Some(Extensions { mcp_config: Some(config), ..Extensions::default() });
    request.host.credentials.mcp_tokens = Some(domain::credentials::McpSignIns(Arc::new(SignedIn)));
    let initialized = json!({ "jsonrpc": "2.0", "id": 1, "result": { "capabilities": { "tools": {} } } });
    let http = Arc::new(ScriptedHttp::new([(200, initialized.to_string()), (202, String::new()), (200, listed(2))]));
    let shared: Arc<dyn Http> = http.clone();
    assert_eq!(tools(&request, &shared).len(), 1);
    assert!(http.sent().iter().all(|s| s.headers.contains(&("Authorization".to_owned(), "Bearer oauth-1".to_owned()))));

    request.host.credentials.mcp_tokens = None;
    let spec = super::servers(request.folder(), &super::McpSources::default()).remove(0);
    let refusing: Arc<dyn Http> = Arc::new(ScriptedHttp::new([(401, String::new())]));
    let error = McpServer::open(&spec, &refusing).expect("open").initialize_stopped(&platform::Stop::new());
    let error = error.expect_err("refused");
    assert!(error.needs_sign_in() && error.to_string().contains("/mcp login linear"), "{error}");
}
