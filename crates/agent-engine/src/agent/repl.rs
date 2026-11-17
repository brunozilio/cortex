use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Instant;

use domain::code_mode::{Declared, Refused, Report};
use domain::{AgentStatus, EngineEvent};
use platform::sync::Locked;
use script::session::{Evaluate, Evaluation, HostCall, Limits, Message as Sent};
use serde_json::{Value, json};

use super::Agent;
use super::tasks::Tasks;
use crate::control::Control;
use crate::tools::repl::{REPL_TOOL, Repl, SEARCH_TOOLS, helpers, reachable, session};
use crate::tools::{Outcome, Tool, Workspace};
use crate::transcript::{Block, Message};

impl Agent<'_> {
    pub(super) fn repl(&self, id: &str, input: &Value, tasks: &Tasks, parent: &[Message]) -> Outcome {
        let owner = format!("{}/{}", self.workspace.session_id, self.workspace.agent_name);
        let context = session(&owner);
        let fresh = domain::code_mode::fresh_context(context.started_now, earlier_scripts(parent, id));
        self.script(input, tasks, parent, &context.session, fresh)
    }

    #[allow(clippy::needless_lifetimes)]
    pub(super) fn repl_in_background<'env>(
        &'env self,
        input: &Value,
        tasks: &Tasks,
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Outcome {
        if input["code"].as_str().is_none_or(|code| code.trim().is_empty()) {
            return Outcome::error("Missing required parameter `code`.");
        }
        let input = input.clone();
        let job = move |control: &Arc<Control>, workspace: &Workspace| {
            let agent = Agent { control, workspace, request: self.request.clone(), ..*self };
            let session = script::session::Session::with_helpers(helpers());
            agent.script(&input, &Tasks::default(), &[], &session, false)
        };
        match self.in_background(tasks, "script", Box::new(job), spawn) {
            Ok(id) => Outcome::ok(
                json!({
                    "task_id": id,
                    "status": "running",
                    "instructions": "The script runs in its own JavaScript context, apart from your REPL variables. \
                                     Collect what it returns with TaskOutput before completing the request; TaskStop stops it."
                })
                .to_string(),
            ),
            Err(refused) => refused,
        }
    }

    fn script(
        &self,
        input: &Value,
        tasks: &Tasks,
        parent: &[Message],
        session: &script::session::Session,
        fresh: bool,
    ) -> Outcome {
        let Some(code) = input["code"].as_str().filter(|code| !code.trim().is_empty()) else {
            return Outcome::error("Missing required parameter `code`.");
        };
        let functions: Vec<String> =
            self.request.tools.iter().map(|spec| spec.name.clone()).filter(|name| reachable(name)).collect();
        let limits = Limits { total: domain::code_mode::total_limit(input["timeout"].as_u64()), ..Limits::default() };
        let request = Evaluate { code: code.to_owned(), functions, limits, stop: Arc::clone(&self.workspace.stop) };
        let refused: Mutex<Vec<Refused>> = Mutex::default();
        let lane: OnceLock<usize> = OnceLock::new();
        let started = Instant::now();
        let watch = |event: EngineEvent| {
            if let EngineEvent::Refused { tool, reason } = &event {
                refused.locked().push(Refused { tool: tool.clone(), reason: reason.clone() });
            }
            if !nested(&event) {
                return (self.emit)(event);
            }
            let index = *lane.get_or_init(|| self.open_lane(&Repl.summary(input), code));
            let asked = matches!(event, EngineEvent::Asked(_)).then(|| event.clone());
            (self.emit)(EngineEvent::AgentEvent { index, event: Box::new(event) });
            asked.into_iter().for_each(self.emit);
        };
        let watching = Agent { emit: &watch, request: self.request.clone(), ..*self };
        let evaluation = {
            let _unwatched = self.workspace.tracking.seen_files.unwatched();
            let model_s_own = std::mem::take(&mut *tasks.repeats.locked());
            let evaluation = watching.serve_script(session.evaluate(request), tasks, parent);
            *tasks.repeats.locked() = model_s_own;
            evaluation
        };
        let report = Report {
            fresh,
            logs: evaluation.logs,
            refused: refused.into_inner().unwrap_or_default(),
            outcome: evaluation.outcome,
        };
        if let Some(&index) = lane.get() {
            let status = match &report.outcome {
                Ok(_) => AgentStatus::Done { seconds: u32::try_from(started.elapsed().as_secs()).unwrap_or(u32::MAX) },
                Err(_) if self.control.is_stopped() => AgentStatus::Cancelled,
                Err(_) => AgentStatus::Failed,
            };
            (self.emit)(EngineEvent::AgentUpdate { index, status, summary: None });
        }
        let text = domain::code_mode::reported(&report, domain::code_mode::PART);
        if report.outcome.is_ok() { Outcome::ok(text) } else { Outcome::error(text) }
    }

    fn open_lane(&self, summary: &str, code: &str) -> usize {
        let index = self.control.next_agent();
        (self.emit)(EngineEvent::AgentSpawned {
            index,
            kind: REPL_TOOL.to_owned(),
            summary: summary.to_owned(),
            prompt: code.to_owned(),
        });
        (self.emit)(EngineEvent::AgentUpdate { index, status: AgentStatus::Running { progress: 0.0 }, summary: None });
        index
    }

    fn serve_script(&self, messages: mpsc::Receiver<Sent>, tasks: &Tasks, parent: &[Message]) -> Evaluation {
        std::thread::scope(|scope| {
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
        })
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

fn nested(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::ToolStarted(_)
            | EngineEvent::ToolFinished { .. }
            | EngineEvent::ToolResult { .. }
            | EngineEvent::Asked(_)
    )
}

fn earlier_scripts(history: &[Message], current: &str) -> usize {
    history
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter(|block| matches!(block, Block::ToolUse { id, name, .. } if name == REPL_TOOL && id != current))
        .count()
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
