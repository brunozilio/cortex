use std::cell::{Cell, RefCell};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::Agent;
use super::tasks::Tasks;
use crate::model::ToolSpec;
use crate::tools::script_bridge::{Bridge, Request};
use crate::tools::script_tool::ScriptTool;
use crate::tools::skill::FoundSkill;
use crate::tools::{Access, Outcome, Tool, Workspace};
use crate::transcript::{Block, Message};

const DEEPEST: usize = 8;

struct Script<'a> {
    name: &'a str,
    path: &'a str,
    source: &'a str,
}

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

struct ScriptCall<'a, 'b> {
    agent: &'a Agent<'b>,
    tool: &'a ScriptTool,
    parent: &'a [Message],
    id: &'a str,
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
        self.agent.run_script(self.id, self.tool, input, self.parent)
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
        let call = ScriptCall { agent: self, tool, parent, id };
        let approved = tool.trusted || self.approved(&tool.spec.name, summary, input);
        self.permitted(id, &call, summary, input, if approved { permission } else { Some("ask") })
    }

    fn approved(&self, name: &str, summary: &str, input: &Value) -> bool {
        matches!(
            self.workspace.ruling(name, summary, input).or_else(|| self.remembered_rule(name, summary, input)),
            Some((domain::permission_rules::RuleKind::Allow, _))
        )
    }

    pub(super) fn project_skill<'t>(&self, tool: &'t dyn Tool, input: &Value) -> Option<&'t FoundSkill> {
        let name = input["skill"].as_str()?.trim_start_matches('/');
        tool.skills().iter().find(|skill| skill.name == name && skill.project_script())
    }

    pub(super) fn skill_call(
        &self,
        id: &str,
        tool: &dyn Tool,
        skill: &FoundSkill,
        summary: &str,
        input: &Value,
        permission: Option<&str>,
    ) -> Outcome {
        let approved = self.approved(&tool.spec().name, summary, input);
        let shown = format!("{summary}, the project's script {}", skill.file.display());
        self.permitted_as(id, tool, summary, Some(&shown), input, if approved { permission } else { Some("ask") })
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

    fn run_script(&self, id: &str, tool: &ScriptTool, input: &Value, parent: &[Message]) -> Outcome {
        let path = tool.path.display().to_string();
        let script = Script { name: &tool.spec.name, path: &path, source: &tool.source };
        self.run_js(id, &script, input, parent, None)
    }

    pub(super) fn execute_declared(
        &self,
        id: &str,
        name: &str,
        input: &Value,
        parent: &[Message],
        builtin: &dyn Fn(&Value) -> Outcome,
    ) -> Outcome {
        let path = crate::tools::declared_path(name);
        let Some(source) = content::source(&path) else { return builtin(input) };
        let script = Script { name, path: &path, source: &source };
        self.run_js(id, &script, input, parent, Some(builtin))
    }

    fn run_js(
        &self,
        id: &str,
        script: &Script<'_>,
        input: &Value,
        parent: &[Message],
        builtin: Option<&dyn Fn(&Value) -> Outcome>,
    ) -> Outcome {
        let depth = DEPTH.with(Cell::get);
        if depth >= DEEPEST {
            return Outcome::error(format!("{} called tools {DEEPEST} levels deep; stopped", script.name));
        }
        let (send, requests) = mpsc::channel::<Request>();
        let host: Arc<dyn script::Host> = Arc::new(Bridge(Mutex::new(send)));
        let data = self.script_context();
        let stop = Arc::clone(&self.workspace.stop);
        let export = if script::exports(script.path, script.source, "execute") { "execute" } else { "run" };
        let (path, source, input) = (script.path.to_owned(), script.source.to_owned(), input.clone());
        let last: RefCell<Option<Outcome>> = RefCell::new(None);
        DEPTH.with(|cell| cell.set(depth + 1));
        let result = std::thread::scope(|scope| {
            let running = scope.spawn(move || script::run(&path, &source, export, &[input], &data, host, stop));
            loop {
                match requests.recv_timeout(Duration::from_millis(25)) {
                    Ok(request) if request.function == "builtin" => {
                        let answer = match builtin {
                            Some(builtin) => {
                                let outcome = builtin(&request.arguments);
                                let content = Value::String(outcome.content.clone());
                                last.replace(Some(outcome));
                                Ok(content)
                            }
                            None => Err(format!("{} has no built-in to call", script.name)),
                        };
                        let _ = request.reply.send(answer);
                    }
                    Ok(request) => {
                        let answer = self.serve(id, &request.function, request.arguments, parent);
                        let _ = request.reply.send(answer);
                    }
                    Err(RecvTimeoutError::Timeout) if !running.is_finished() => {}
                    Err(_) => break,
                }
            }
            running.join().unwrap_or_else(|_| Err("the script crashed".into()))
        });
        DEPTH.with(|cell| cell.set(depth));
        let last = last.into_inner();
        match (result, last) {
            (Ok(Value::String(text)), Some(outcome)) if text == outcome.content => outcome,
            (Err(_), Some(outcome)) if self.workspace.stop.load(std::sync::atomic::Ordering::Relaxed) => outcome,
            (Ok(Value::String(text)), _) => Outcome::ok(text),
            (Ok(Value::Null), _) => Outcome::ok(""),
            (Ok(value), _) => Outcome::ok(value.to_string()),
            (Err(error), _) => Outcome::error(format!("{} failed: {error}", script.name)),
        }
    }

    fn serve(&self, id: &str, function: &str, arguments: Value, parent: &[Message]) -> Result<Value, String> {
        let access = self.workspace.policy.scripts;
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
                let text = arguments.as_str().map_or_else(|| arguments.to_string(), str::to_owned);
                (self.emit)(domain::EngineEvent::ToolLog { id: id.to_owned(), text });
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
        crate::tools::script_bridge::ask_model(self.api, &self.request, arguments, self.control.stop_flag())
    }
}
