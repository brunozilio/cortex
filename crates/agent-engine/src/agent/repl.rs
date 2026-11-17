use std::sync::Arc;

use domain::code_mode::Declared;
use script::session::{Evaluate, Evaluation, HostCall, Limits, Message as Sent};
use serde_json::{Value, json};

use super::Agent;
use super::tasks::Tasks;
use crate::tools::Outcome;
use crate::tools::repl::{REPL_TOOL, SEARCH_TOOLS, reachable, session};
use crate::transcript::{Block, Message};

impl Agent<'_> {
    pub(super) fn repl(&self, input: &Value, tasks: &Tasks, parent: &[Message]) -> Outcome {
        let Some(code) = input["code"].as_str().filter(|code| !code.trim().is_empty()) else {
            return Outcome::error("Missing required parameter `code`.");
        };
        let functions: Vec<String> =
            self.request.tools.iter().map(|spec| spec.name.clone()).filter(|name| reachable(name)).collect();
        let limits = Limits { total: domain::code_mode::total_limit(input["timeout"].as_u64()), ..Limits::default() };
        let request = Evaluate { code: code.to_owned(), functions, limits, stop: Arc::clone(&self.workspace.stop) };
        let owner = format!("{}/{}", self.workspace.session_id, self.workspace.agent_name);
        let messages = session(&owner).evaluate(request);
        let evaluation = std::thread::scope(|scope| {
            for message in messages.iter() {
                match message {
                    Sent::Done(evaluation) => return evaluation,
                    Sent::Call(call) if self.concurrent(&call.function, &call.input) => {
                        scope.spawn(move || self.answer(call, tasks, &[]));
                    }
                    Sent::Call(call) => self.answer(call, tasks, parent),
                }
            }
            Evaluation { logs: Vec::new(), outcome: Err("the script's context stopped answering".into()), calls: 0 }
        });
        let shown = domain::code_mode::shown(&evaluation.logs, &evaluation.outcome, domain::code_mode::PART);
        if evaluation.outcome.is_ok() { Outcome::ok(shown) } else { Outcome::error(shown) }
    }

    fn answer(&self, call: HostCall, tasks: &Tasks, parent: &[Message]) {
        if call.function == SEARCH_TOOLS {
            let found = self.search_tools(&call.input);
            return call.reply.send(found);
        }
        if !reachable(&call.function) {
            let refusal = format!("{} cannot be called from a {REPL_TOOL} script; call it directly", call.function);
            return call.reply.send(Err(refusal));
        }
        let id = format!("repl-{}", self.control.next_agent());
        let spawn = |job: Box<dyn FnOnce() + Send + '_>| job();
        let block = self.call(&id, &call.function, &call.input, tasks, parent, &spawn);
        call.reply.send(answered(block));
    }

    fn search_tools(&self, input: &Value) -> Result<Value, String> {
        let query = input["query"].as_str().map(str::trim).filter(|query| !query.is_empty());
        let query = query.ok_or("searchTools needs the words to look for, e.g. searchTools('slack message')")?;
        let Some(deferred) = &self.workspace.services.deferred else {
            return Ok(json!([]));
        };
        let found: Vec<Value> = deferred
            .search(query)
            .into_iter()
            .map(|spec| {
                let declared = Declared { name: &spec.name, description: &spec.description, schema: &spec.schema };
                json!({ "name": spec.name, "declaration": domain::code_mode::declaration(&declared) })
            })
            .collect();
        Ok(Value::Array(found))
    }
}

fn answered(block: Block) -> Result<Value, String> {
    match block {
        Block::ToolResult { content, is_error: false, .. } | Block::ToolResultRich { content, is_error: false, .. } => {
            Ok(Value::String(content))
        }
        Block::ToolResult { content, .. } | Block::ToolResultRich { content, .. } => Err(content),
        other => Err(format!("the tool answered with {other:?}")),
    }
}
