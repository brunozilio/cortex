use superfik_core::{MemoryBrief, MemoryTool, ModelOption};

use super::*;
use crate::model::http::ScriptedHttp;
use crate::tools::{Access, Workspace};

fn listed(id: u64) -> String {
    json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "tools": [{
            "name": "memory_search",
            "description": "Searches the project's memory.",
            "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } },
        }] },
    })
    .to_string()
}

fn memory_request() -> PromptRequest {
    let model = ModelOption::new("m", "M");
    let mut request = PromptRequest::plain("/repo".into(), "hi", model);
    let tool = MemoryTool { url: "http://127.0.0.1:7/mcp".into(), token: "t-1".into() };
    request.memory = Some(MemoryBrief { tool: Some(tool), ..MemoryBrief::default() });
    request
}

#[test]
fn a_granted_server_s_tools_are_named_for_it_and_called_with_its_token() {
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
    let tools = tools(&memory_request(), &shared);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].spec().name, "mcp__superfik_memory__memory_search");
    assert_eq!(tools[0].access(), Access::Reads, "the app's own run without asking");
    assert_eq!(tools[0].spec().schema["properties"]["query"]["type"], "string");
    let input = json!({ "query": "renderer" });
    assert_eq!(tools[0].summary(&input), "renderer");
    assert_eq!(tools[0].run(&input, &Workspace::new("/repo")), Outcome::ok("[4] Uses wgpu"));
    let sent = http.sent();
    assert!(sent.iter().all(|s| s.url == "http://127.0.0.1:7/mcp"));
    assert!(sent.iter().all(|s| s.headers.contains(&("authorization".to_owned(), "Bearer t-1".to_owned()))));
    let call: Value = serde_json::from_str(&sent[3].body).expect("json");
    assert_eq!((call["method"].as_str(), call["params"]["name"].as_str()), (Some("tools/call"), Some("memory_search")));
}

#[test]
fn a_failed_call_and_an_unreachable_server_say_so() {
    let failed = json!({ "jsonrpc": "2.0", "id": 1, "result": { "content": [{ "type": "text", "text": "no" }], "isError": true } });
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([(200, failed.to_string()), (401, String::new())]));
    let spec = servers(&memory_request(), &super::McpSources::default()).remove(0);
    let server = McpServer::open(&spec, &http).expect("open");
    assert_eq!(
        server.call("memory_search", &Value::Null, &std::sync::atomic::AtomicBool::new(false)),
        Outcome::error("no")
    );
    assert!(
        server
            .call("memory_search", &Value::Null, &std::sync::atomic::AtomicBool::new(false))
            .content
            .contains("HTTP 401")
    );
    let none: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    assert!(tools(&memory_request(), &none).is_empty(), "the run goes on without it");
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
    };
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let server = Arc::new(McpServer::open(&spec, &http).expect("spawn"));
    let tools = server.tools().expect("tools");
    assert_eq!(tools[0].spec().name, "mcp__echo-me__echo");
    assert_eq!(tools[0].access(), Access::Executes, "a host's tool is asked for");
    assert_eq!(tools[0].run(&json!({}), &Workspace::new("/")), Outcome::ok("hi there"));
}

#[test]
fn a_command_that_does_not_exist_is_left_out() {
    let spec =
        ServerSpec::Stdio { name: "gone".into(), command: "superfik-no-such-tool".into(), args: vec![], env: vec![] };
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let error = McpServer::open(&spec, &http).expect_err("missing");
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
    assert!(error.contains("repeated"));
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

struct Echo;

impl superfik_core::mcp::McpEndpoint for Echo {
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
    request.credentials.mcp = vec![superfik_core::mcp::LocalMcp::new("host", Arc::new(Echo))];
    let http = Arc::new(ScriptedHttp::new([]));
    let shared: Arc<dyn Http> = http.clone();
    let tools = tools(&request, &shared);
    assert_eq!(tools.iter().map(|t| t.spec().name).collect::<Vec<_>>(), ["mcp__host__echo"]);
    assert_eq!(tools[0].run(&json!({ "say": "hello" }), &Workspace::new("/repo")), Outcome::ok("hello"));
    assert!(http.sent().is_empty(), "nothing went over HTTP");
}
