use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, mpsc};

use serde_json::Value;

use crate::model::{ModelApi, TurnRequest};
use crate::tools::{Effect, Workspace, changes_nothing};
use crate::transcript::Message;

pub(crate) struct Request {
    pub function: String,
    pub arguments: Value,
    pub reply: mpsc::Sender<Result<Value, String>>,
}

pub(crate) struct Bridge(pub Mutex<mpsc::Sender<Request>>);

impl script::Host for Bridge {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        let (reply, answer) = mpsc::channel();
        let request = Request { function: function.to_owned(), arguments, reply };
        self.0.lock().map_err(|_| "the agent is unavailable")?.send(request).map_err(|_| "the agent is gone")?;
        answer.recv().map_err(|_| "the agent did not answer".to_owned())?
    }
}

pub(crate) fn ask_model(
    api: &dyn ModelApi,
    template: &TurnRequest,
    arguments: &Value,
    stop: &AtomicBool,
) -> Result<Value, String> {
    let field = |key: &str| arguments.get(key).and_then(Value::as_str).map(str::to_owned);
    let prompt = field("prompt").ok_or("ctx.llm needs a prompt")?;
    let request = TurnRequest {
        model: field("model").unwrap_or_else(|| template.model.clone()),
        system: field("system").unwrap_or_default(),
        messages: vec![Message::user(prompt)],
        tools: Vec::new(),
        ..template.clone()
    };
    let reply = api.turn(&request, &mut |_| {}, stop).map_err(|error| error.to_string())?;
    Ok(Value::String(reply.message.text()))
}

pub(crate) fn read_only_tool(arguments: &Value, workspace: &Workspace) -> Result<Value, String> {
    let name = arguments["name"].as_str().ok_or("ctx.tool needs a tool's name")?;
    let input = &arguments["input"];
    let tools = crate::tools::standard();
    let only_reads = || format!("a hook may run only read-only tools such as Read, Grep and Glob, not {name}");
    let (tool, spec) =
        tools.iter().map(|tool| (tool, tool.spec())).find(|(_, spec)| spec.name == name).ok_or_else(only_reads)?;
    if let Some(refusal) = crate::tools::checked::refusal(name, &spec.schema, input) {
        return Err(refusal);
    }
    if !changes_nothing(&**tool, input, workspace) {
        return Err(only_reads());
    }
    let outside = tool.effects(input, workspace).into_iter().find_map(|effect| match effect {
        Effect::ReadPath(place) | Effect::ReadTree(place) if !workspace.bounds().may_read(&place.real) => Some(place),
        _ => None,
    });
    if let Some(outside) = outside {
        return Err(format!("a hook's {name} stays inside the project; {} is outside", outside.written.display()));
    }
    let outcome = tool.run(input, workspace);
    if outcome.is_error { Err(outcome.content) } else { Ok(Value::String(outcome.content)) }
}
