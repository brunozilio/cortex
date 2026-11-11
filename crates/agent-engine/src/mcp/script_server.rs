use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use domain::mcp::{McpEndpoint, respond, text_result};
use serde_json::{Value, json};

use super::ServerSpec;
use crate::tools::script_io::StandaloneHost;

pub fn found(folder: &Path, cwd: &Path) -> Vec<ServerSpec> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut servers: Vec<ServerSpec> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            Some(ServerSpec::Script { name, path, cwd: cwd.to_path_buf() })
        })
        .collect();
    servers.sort_by(|a, b| a.name().cmp(b.name()));
    servers
}

pub struct ScriptServer {
    name: String,
    path: PathBuf,
    source: String,
    cwd: PathBuf,
    tools: Vec<Value>,
}

impl std::fmt::Debug for ScriptServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptServer").field("name", &self.name).field("path", &self.path).finish_non_exhaustive()
    }
}

impl ScriptServer {
    pub fn open(name: &str, path: &Path, cwd: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let shown = path.display().to_string();
        let exported = script::exported(&shown, &source, "default").map_err(|error| format!("{shown}: {error}"))?;
        let tools = exported["tools"]
            .as_object()
            .ok_or_else(|| format!("{shown}: export default {{ tools: {{ … }} }}"))?
            .iter()
            .map(|(tool, spec)| {
                json!({
                    "name": tool,
                    "description": spec["description"].as_str().unwrap_or_default(),
                    "inputSchema": spec.get("input").cloned().unwrap_or_else(|| json!({ "type": "object" })),
                })
            })
            .collect();
        Ok(Self { name: name.to_owned(), path: path.to_path_buf(), source, cwd: cwd.to_path_buf(), tools })
    }

    fn call(&self, tool: &str, arguments: &Value) -> Value {
        if !self.tools.iter().any(|known| known["name"] == tool) {
            return text_result(format!("{} has no tool {tool}", self.name), true);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let host: Arc<dyn script::Host> =
            Arc::new(StandaloneHost { cwd: self.cwd.clone(), access: Default::default(), stop: Arc::clone(&stop) });
        let data = json!({ "cwd": self.cwd.display().to_string(), "server": self.name });
        let export = format!("default.tools.{tool}.run");
        let shown = self.path.display().to_string();
        match script::run(&shown, &self.source, &export, std::slice::from_ref(arguments), &data, host, stop) {
            Ok(Value::String(text)) => text_result(text, false),
            Ok(Value::Null) => text_result("", false),
            Ok(value) => text_result(value.to_string(), false),
            Err(error) => text_result(error, true),
        }
    }
}

impl McpEndpoint for ScriptServer {
    fn handle(&self, message: &Value) -> Option<Value> {
        respond(message, &self.name, || self.tools.clone(), |tool, arguments| Ok(self.call(tool, arguments)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_javascript_server_lists_and_runs_its_tools() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("math.js");
        std::fs::write(
            &file,
            r#"export default { tools: {
  add: { description: "Adds a and b.", input: { type: "object" }, run: (input) => String(input.a + input.b) },
  fail: { run: () => { throw new Error("nope") } },
} };"#,
        )
        .expect("server");
        let server = ScriptServer::open("math", &file, dir.path()).expect("open");
        let listed = server.handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).expect("answer");
        assert_eq!(listed["result"]["tools"][0]["name"], "add");
        let call = |name: &str| json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": { "a": 2, "b": 3 } } });
        let added = server.handle(&call("add")).expect("answer");
        assert_eq!(added["result"]["content"][0]["text"], "5");
        let failed = server.handle(&call("fail")).expect("answer");
        assert_eq!(failed["result"]["isError"], true);
        assert_eq!(found(dir.path(), dir.path()).len(), 1);
    }
}
