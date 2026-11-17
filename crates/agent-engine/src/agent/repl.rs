use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use domain::code_mode::{Declared, Refused, Report};
use domain::{AgentStatus, EngineEvent};
use platform::sync::Locked;
use script::session::{Evaluate, Evaluation, HostCall, Limits, Message as Sent};
use serde_json::{Value, json};

use super::Agent;
use super::tasks::Tasks;
use crate::control::Control;
use crate::tools::repl::{READ_TEXT, REPL_TOOL, Repl, SEARCH_TOOLS, reachable, session};
use crate::tools::{Outcome, Tool, Workspace, raw_text};
use crate::transcript::{Block, Message};

const POLL: Duration = Duration::from_millis(25);

pub(super) struct ScriptRun {
    number: u64,
    messages: mpsc::Receiver<Sent>,
    stop: Arc<AtomicBool>,
    refused: Mutex<Vec<Refused>>,
    lane: OnceLock<usize>,
    started: Instant,
    summary: String,
    code: String,
    fresh: bool,
}

impl Agent<'_> {
    #[allow(clippy::needless_lifetimes)]
    pub(super) fn repl<'env>(
        &'env self,
        id: &str,
        input: &Value,
        tasks: &Tasks,
        parent: &[Message],
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Outcome {
        let Some(code) = input["code"].as_str().filter(|code| !code.trim().is_empty()) else {
            return Outcome::error("Missing required parameter `code`.");
        };
        let owner = format!("{}/{}", self.workspace.session_id, self.workspace.agent_name);
        let context = session(&owner);
        let functions: Vec<String> =
            self.request.tools.iter().map(|spec| spec.name.clone()).filter(|name| reachable(name)).collect();
        let limits = Limits { total: domain::code_mode::total_limit(input["timeout"].as_u64()), ..Limits::default() };
        let stop = Arc::new(AtomicBool::new(false));
        let submitted =
            context.session.submit(Evaluate { code: code.to_owned(), functions, limits, stop: Arc::clone(&stop) });
        let run = ScriptRun {
            number: submitted.number,
            messages: submitted.messages,
            stop,
            refused: Mutex::default(),
            lane: OnceLock::new(),
            started: Instant::now(),
            summary: Repl.summary(input),
            code: code.to_owned(),
            fresh: domain::code_mode::fresh_context(context.started_now, earlier_scripts(parent, id)),
        };
        let detach = self.workspace.policy.code_mode_detach.unwrap_or(domain::code_mode::DETACH);
        match self.serving(&run, tasks, parent, Some(Instant::now() + detach)) {
            Some(evaluation) => run.report(self, evaluation),
            None if self.can_background(tasks) => self.dispatch(run, tasks, detach, spawn),
            None => {
                let evaluation = self.serving(&run, tasks, parent, None).unwrap_or_else(|| gone(run.number));
                run.report(self, evaluation)
            }
        }
    }

    #[allow(clippy::needless_lifetimes)]
    fn dispatch<'env>(
        &'env self,
        run: ScriptRun,
        tasks: &Tasks,
        detach: Duration,
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Outcome {
        let number = run.number;
        let job = move |control: &Arc<Control>, _forked: &Workspace| {
            let agent = Agent { control, request: self.request.clone(), ..*self };
            let evaluation = agent.serving(&run, &Tasks::default(), &[], None).unwrap_or_else(|| gone(number));
            let mut outcome = run.report(&agent, evaluation);
            outcome.content = format!("Script #{number} ended.\n{}", outcome.content);
            outcome
        };
        match self.in_background(tasks, "script", Box::new(job), spawn) {
            Ok(task) => Outcome::ok(domain::code_mode::dispatched(number, &task, detach)),
            Err(refused) => refused,
        }
    }

    fn serving(
        &self,
        run: &ScriptRun,
        tasks: &Tasks,
        parent: &[Message],
        until: Option<Instant>,
    ) -> Option<Evaluation> {
        let _unwatched = self.workspace.tracking.seen_files.unwatched();
        let model_s_own = std::mem::take(&mut *tasks.repeats.locked());
        let served = run.serve(self, tasks, parent, until);
        *tasks.repeats.locked() = model_s_own;
        served
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

    fn answer(&self, call: HostCall, tasks: &Tasks, parent: &[Message]) {
        if call.function == SEARCH_TOOLS {
            let found = self.search_tools(&call.input);
            return call.reply.send(found);
        }
        if call.function == READ_TEXT {
            let input = json!({ "file_path": call.input["path"] });
            let id = format!("repl-{}", self.control.next_agent());
            let spawn = |job: Box<dyn FnOnce() + Send + '_>| job();
            let block = raw_text::reading(|| self.call(&id, "Read", &input, tasks, parent, &spawn));
            return call.reply.send(answered(block, "Read"));
        }
        if !reachable(&call.function) {
            let refusal = format!("{} cannot be called from a {REPL_TOOL} script; call it directly", call.function);
            return call.reply.send(Err(refusal));
        }
        let id = format!("repl-{}", self.control.next_agent());
        let spawn = |job: Box<dyn FnOnce() + Send + '_>| job();
        let block = self.call(&id, &call.function, &call.input, tasks, parent, &spawn);
        call.reply.send(answered(block, &call.function));
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

impl ScriptRun {
    fn serve(
        &self,
        agent: &Agent<'_>,
        tasks: &Tasks,
        parent: &[Message],
        until: Option<Instant>,
    ) -> Option<Evaluation> {
        let (refused, lane, summary, code) = (&self.refused, &self.lane, &self.summary, &self.code);
        let watch = |event: EngineEvent| {
            if let EngineEvent::Refused { tool, reason } = &event {
                refused.locked().push(Refused { tool: tool.clone(), reason: reason.clone() });
            }
            if !nested(&event) {
                return (agent.emit)(event);
            }
            let index = *lane.get_or_init(|| agent.open_lane(summary, code));
            let asked = matches!(event, EngineEvent::Asked(_)).then(|| event.clone());
            (agent.emit)(EngineEvent::AgentEvent { index, event: Box::new(event) });
            asked.into_iter().for_each(agent.emit);
        };
        let watching = Agent { emit: &watch, request: agent.request.clone(), ..*agent };
        let watching = &watching;
        std::thread::scope(|scope| {
            loop {
                if agent.control.is_stopped() {
                    self.stop.store(true, Ordering::Relaxed);
                }
                let wait = match until {
                    Some(until) if Instant::now() >= until => return None,
                    Some(until) => until.saturating_duration_since(Instant::now()).min(POLL),
                    None => POLL,
                };
                match self.messages.recv_timeout(wait) {
                    Ok(Sent::Done(evaluation)) => return Some(evaluation),
                    Ok(Sent::Call(call)) if watching.concurrent(&call.function, &call.input) => {
                        scope.spawn(move || watching.answer(call, tasks, &[]));
                    }
                    Ok(Sent::Call(call)) => watching.answer(call, tasks, parent),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Some(gone(self.number)),
                }
            }
        })
    }

    fn report(&self, agent: &Agent<'_>, evaluation: Evaluation) -> Outcome {
        let report = Report {
            fresh: self.fresh,
            logs: evaluation.logs,
            refused: std::mem::take(&mut *self.refused.locked()),
            outcome: evaluation.outcome,
        };
        if let Some(&index) = self.lane.get() {
            let status = match &report.outcome {
                Ok(_) => {
                    AgentStatus::Done { seconds: u32::try_from(self.started.elapsed().as_secs()).unwrap_or(u32::MAX) }
                }
                Err(_) if agent.control.is_stopped() || self.stop.load(Ordering::Relaxed) => AgentStatus::Cancelled,
                Err(_) => AgentStatus::Failed,
            };
            (agent.emit)(EngineEvent::AgentUpdate { index, status, summary: None });
        }
        let text = domain::code_mode::reported(&report, domain::code_mode::PART);
        if report.outcome.is_ok() { Outcome::ok(text) } else { Outcome::error(text) }
    }
}

fn gone(number: u64) -> Evaluation {
    Evaluation { number, logs: Vec::new(), outcome: Err("the script's context stopped answering".into()), calls: 0 }
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

fn answered(block: Block, tool: &str) -> Result<Value, String> {
    match block {
        Block::ToolResult { content, is_error: false, .. } | Block::ToolResultRich { content, is_error: false, .. } => {
            Ok(Value::String(content))
        }
        Block::ToolResult { content, .. } | Block::ToolResultRich { content, .. }
            if domain::code_mode::throws_on_failure(tool) =>
        {
            Err(content)
        }
        Block::ToolResult { content, .. } | Block::ToolResultRich { content, .. } => Ok(json!({ "error": content })),
        other => Err(format!("the tool answered with {other:?}")),
    }
}
