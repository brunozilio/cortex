use std::cell::Cell;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::Agent;
use super::tasks::Tasks;
use crate::model::{ToolSpec, TurnRequest};
use crate::tools::script_tool::ScriptTool;
use crate::tools::{Access, Outcome, Tool, Workspace};
use crate::transcript::{Block, Message};

const DEEPEST: usize = 8;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

struct Request {
    function: String,
    arguments: Value,
    reply: mpsc::Sender<Result<Value, String>>,
}

struct Bridge(Mutex<mpsc::Sender<Request>>);

impl script::Host for Bridge {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        let (reply, answer) = mpsc::channel();
        let request = Request { function: function.to_owned(), arguments, reply };
        self.0.lock().map_err(|_| "the agent is unavailable")?.send(request).map_err(|_| "the agent is gone")?;
        answer.recv().map_err(|_| "the agent did not answer".to_owned())?
    }
}

struct ScriptCall<'a, 'b> {
    agent: &'a Agent<'b>,
    tool: &'a ScriptTool,
    parent: &'a [Message],
}

impl Tool for ScriptCall<'_, '_> {
    fn spec(&self) -> ToolSpec {
        self.tool.spec.clone()
    }

    fn access(&self) -> Access {
        self.tool.access
    }

    fn summary(&self, input: &Value) -> String {
        self.tool.summary(input)
    }

    fn run(&self, input: &Value, _workspace: &Workspace) -> Outcome {
        self.agent.run_script(self.tool, input, self.parent)
    }
}

fn answered(block: Block) -> Result<Value, String> {
    match block {
        Block::ToolResult { content, is_error: false, .. } | Block::ToolResultRich { content, is_error: false, .. } => {
            Ok(Value::String(content))
        }
        Block::ToolResult { content, .. } | Block::ToolResultRich { content, .. } => Err(content),
        other => Ok(json!(format!("{other:?}"))),
    }
}

impl Agent<'_> {
    pub(super) fn script_call(
        &self,
        id: &str,
        tool: &ScriptTool,
        summary: &str,
        input: &Value,
        parent: &[Message],
        permission: Option<&str>,
    ) -> Outcome {
        let call = ScriptCall { agent: self, tool, parent };
        self.permitted(id, &call, summary, input, permission)
    }

    fn script_context(&self) -> Value {
        let skills: Vec<Value> = self
            .tools
            .iter()
            .flat_map(|tool| tool.skills())
            .map(|skill| json!({ "name": skill.name, "description": skill.description, "path": skill.file.display().to_string() }))
            .collect();
        json!({
            "cwd": self.workspace.cwd.display().to_string(),
            "model": self.request.model,
            "mode": format!("{:?}", self.control.mode()).to_ascii_lowercase(),
            "session": self.workspace.session_id,
            "tools": self.tools.iter().map(|tool| tool.spec().name).collect::<Vec<_>>(),
            "skills": skills,
            "agents": self.agents.iter().map(|agent| json!({ "name": agent.name, "description": agent.description })).collect::<Vec<_>>(),
        })
    }

    fn run_script(&self, tool: &ScriptTool, input: &Value, parent: &[Message]) -> Outcome {
        let depth = DEPTH.with(Cell::get);
        if depth >= DEEPEST {
            return Outcome::error(format!("{} called tools {DEEPEST} levels deep; stopped", tool.spec.name));
        }
        let (send, requests) = mpsc::channel::<Request>();
        let host: Arc<dyn script::Host> = Arc::new(Bridge(Mutex::new(send)));
        let data = self.script_context();
        let stop = Arc::clone(&self.workspace.stop);
        let (path, source, input) = (tool.path.display().to_string(), tool.source.clone(), input.clone());
        DEPTH.with(|cell| cell.set(depth + 1));
        let result = std::thread::scope(|scope| {
            let running = scope.spawn(move || script::run(&path, &source, "run", &[input], &data, host, stop));
            loop {
                match requests.recv_timeout(Duration::from_millis(25)) {
                    Ok(request) => {
                        let answer = self.serve(&request.function, request.arguments, parent);
                        let _ = request.reply.send(answer);
                    }
                    Err(RecvTimeoutError::Timeout) if !running.is_finished() => {}
                    Err(_) => break,
                }
            }
            running.join().unwrap_or_else(|_| Err("the script crashed".into()))
        });
        DEPTH.with(|cell| cell.set(depth));
        match result {
            Ok(Value::String(text)) => Outcome::ok(text),
            Ok(Value::Null) => Outcome::ok(""),
            Ok(value) => Outcome::ok(value.to_string()),
            Err(error) => Outcome::error(format!("{} failed: {error}", tool.spec.name)),
        }
    }

    fn serve(&self, function: &str, arguments: Value, parent: &[Message]) -> Result<Value, String> {
        let access = self.workspace.scripts;
        let text = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        match function {
            "tool" => {
                let name = text("name");
                let id = format!("script-{}", self.control.next_agent());
                let spawn = |job: Box<dyn FnOnce() + Send + '_>| job();
                answered(self.call(&id, &name, &arguments["input"], &Tasks::default(), parent, &spawn))
            }
            "llm" => self.script_llm(&arguments),
            "text" => Ok(Value::String(content::text(arguments.as_str().unwrap_or_default()))),
            "log" => {
                tracing::info!(message = arguments.as_str().unwrap_or_default(), "script");
                Ok(Value::Null)
            }
            "readFile" if access.files => {
                let path = self.workspace.resolve(arguments.as_str().unwrap_or_default());
                std::fs::read_to_string(&path)
                    .map(Value::String)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            "writeFile" if access.files => {
                let path = self.workspace.resolve(&text("path"));
                self.workspace.before_write(&path);
                std::fs::write(&path, text("text"))
                    .map(|()| Value::Null)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            "exec" if access.processes => {
                crate::tools::script_io::exec(&self.workspace.cwd, &arguments, &self.workspace.stop)
            }
            "fetch" if access.network => crate::tools::script_io::fetch(&arguments),
            "readFile" | "writeFile" => Err("scripts may not touch files here (settings: scripts.files)".into()),
            "exec" => Err("scripts may not start processes here (settings: scripts.processes)".into()),
            "fetch" => Err("scripts may not reach the network here (settings: scripts.network)".into()),
            other => Err(format!("ctx.{other} is not a function")),
        }
    }

    fn script_llm(&self, arguments: &Value) -> Result<Value, String> {
        let field = |key: &str| arguments.get(key).and_then(Value::as_str).map(str::to_owned);
        let prompt = field("prompt").ok_or("ctx.llm needs a prompt")?;
        let request = TurnRequest {
            model: field("model").unwrap_or_else(|| self.request.model.clone()),
            system: field("system").unwrap_or_default(),
            messages: vec![Message::user(prompt)],
            tools: Vec::new(),
            ..self.request.clone()
        };
        let reply =
            self.api.turn(&request, &mut |_| {}, self.control.stop_flag()).map_err(|error| error.to_string())?;
        Ok(Value::String(reply.message.text()))
    }
}
