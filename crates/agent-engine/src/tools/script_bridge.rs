use std::sync::{Arc, Mutex};

use crossbeam_channel::{Sender, bounded, unbounded};

use platform::Stop;
use serde_json::Value;

use crate::model::{BlockingTurn, ModelApi, TurnRequest};
use crate::tools::{Effect, Workspace, changes_nothing};
use crate::transcript::Message;

struct Request {
    function: String,
    arguments: Value,
    reply: Sender<Result<Value, String>>,
}

enum Hosted {
    Call(Request),
    Done,
}

struct Bridge(Mutex<Sender<Hosted>>);

impl script::Host for Bridge {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        let (reply, answer) = bounded(1);
        let request = Hosted::Call(Request { function: function.to_owned(), arguments, reply });
        self.0.lock().map_err(|_| "the agent is unavailable")?.send(request).map_err(|_| "the agent is gone")?;
        answer.recv().map_err(|_| "the agent did not answer".to_owned())?
    }
}

pub(crate) fn hosted(
    crashed: &str,
    run: impl FnOnce(Arc<dyn script::Host>) -> Result<Value, String> + Send,
    mut serve: impl FnMut(&str, Value) -> Result<Value, String>,
) -> Result<Value, String> {
    let (send, calls) = unbounded::<Hosted>();
    let finished = send.clone();
    let host: Arc<dyn script::Host> = Arc::new(Bridge(Mutex::new(send)));
    std::thread::scope(|scope| {
        let running = scope.spawn(move || {
            let ran = run(host);
            let _ = finished.send(Hosted::Done);
            ran
        });
        for hosted in calls.iter() {
            let Hosted::Call(request) = hosted else { break };
            let answer = serve(&request.function, request.arguments);
            let _ = request.reply.send(answer);
        }
        running.join().unwrap_or_else(|_| Err(crashed.to_owned()))
    })
}

pub(crate) fn ask_model(
    api: &dyn ModelApi,
    template: &TurnRequest,
    arguments: &Value,
    stop: &Stop,
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
    let reply = api.turn_blocking(&request, &mut |_| {}, stop.token()).map_err(|error| error.to_string())?;
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

#[cfg(test)]
#[path = "script_bridge_tests.rs"]
mod tests;
