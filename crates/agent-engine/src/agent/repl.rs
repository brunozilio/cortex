use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use domain::code_mode::{Declared, Refused, Report};
use domain::{AgentStatus, EngineEvent};
use platform::sync::Locked;
use script::session::{Evaluate, Evaluation, HostCall, Limits, Message as Sent};
use serde_json::{Value, json};

use super::Agent;
use super::run::ThisRun as Tasks;
use crate::control::Control;
use crate::tools::repl::{DESCRIBE_TOOL, READ_TEXT, REPL_TOOL, Repl, SEARCH_TOOLS, reachable, session};
use crate::tools::{Outcome, Tool, Workspace, raw_text};
use crate::transcript::{Block, Message};

const POLL: Duration = Duration::from_millis(25);

pub(super) struct ScriptRun {
    number: u64,
    messages: mpsc::Receiver<Sent>,
    guarded: Guarded,
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
        let limits = Limits {
            running: domain::code_mode::running_limit(input["timeout"].as_u64()),
            total: domain::code_mode::WALL,
            ..Limits::default()
        };
        let stop = Arc::new(AtomicBool::new(false));
        let submitted =
            context.session.submit(Evaluate { code: code.to_owned(), functions, limits, stop: Arc::clone(&stop) });
        let run = ScriptRun {
            number: submitted.number,
            messages: submitted.messages,
            guarded: Guarded {
                stop,
                stopped_by: Mutex::default(),
                carried: Mutex::default(),
                images: Mutex::default(),
            },
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
            if let Some(from) = outcome.carried_from.take() {
                outcome.content = domain::untrusted::fenced(&outcome.content, &from);
            }
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

    fn answer(
        &self,
        function: &str,
        input: &Value,
        (tasks, parent): (&Tasks, &[Message]),
        images: &Mutex<Vec<domain::Image>>,
    ) -> Result<Value, String> {
        if function == SEARCH_TOOLS {
            return self.search_tools(input);
        }
        if function == DESCRIBE_TOOL {
            return self.describe_tool(input);
        }
        if function == READ_TEXT {
            let input = json!({ "file_path": input["path"] });
            let id = format!("repl-{}", self.control.next_agent());
            let spawn = |job: Box<dyn FnOnce() + Send + '_>| job();
            let block = raw_text::reading(|| self.call(&id, "Read", &input, tasks, parent, &spawn));
            return answered(block, "Read", &input);
        }
        if !reachable(function) {
            return Err(format!("{function} cannot be called from a {REPL_TOOL} script; call it directly"));
        }
        let id = format!("repl-{}", self.control.next_agent());
        let spawn = |job: Box<dyn FnOnce() + Send + '_>| job();
        let block = self.call(&id, function, input, tasks, parent, &spawn);
        if let Block::ToolResultRich { images: shown, .. } = &block {
            let mut kept = images.locked();
            let room = domain::code_mode::IMAGES.saturating_sub(kept.len());
            kept.extend(shown.iter().take(room).cloned());
        }
        answered(block, function, input)
    }

    fn describe_tool(&self, input: &Value) -> Result<Value, String> {
        let name = input["name"].as_str().map(str::trim).unwrap_or_default();
        let deferred =
            self.workspace.services.deferred.as_ref().map(|deferred| deferred.search(&format!("select:{name}")));
        let spec = self
            .request
            .tools
            .iter()
            .chain(deferred.iter().flatten())
            .find(|spec| spec.name == name && reachable(&spec.name));
        let Some(spec) = spec else {
            return Ok(Value::String(format!(
                "No tool is named {name}; searchTools(words) finds tools by what they do."
            )));
        };
        let declared = Declared { name: &spec.name, description: &spec.description, schema: &spec.schema };
        Ok(Value::String(domain::code_mode::declaration(&declared)))
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
        let guarded = &self.guarded;
        let asking: Mutex<Vec<(domain::AskId, String)>> = Mutex::default();
        let watch = |event: EngineEvent| {
            if let EngineEvent::Refused { tool, reason } = &event {
                refused.locked().push(Refused { tool: tool.clone(), reason: reason.clone() });
            }
            guarded.watch_prompts(&event, &asking);
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
                    self.guarded.stop.store(true, Ordering::Relaxed);
                }
                let wait = match until {
                    Some(until) if Instant::now() >= until => return None,
                    Some(until) => until.saturating_duration_since(Instant::now()).min(POLL),
                    None => POLL,
                };
                match self.messages.recv_timeout(wait) {
                    Ok(Sent::Done(evaluation)) => return Some(evaluation),
                    Ok(Sent::Call(call)) if watching.concurrent(tasks, &call.function, &call.input) => {
                        scope.spawn(move || {
                            guarded.settle(call, |function, input| {
                                watching.answer(function, input, (tasks, &[]), &guarded.images)
                            });
                        });
                    }
                    Ok(Sent::Call(call)) => {
                        guarded.settle(call, |function, input| {
                            watching.answer(function, input, (tasks, parent), &guarded.images)
                        });
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Some(gone(self.number)),
                }
            }
        })
    }

    fn report(&self, agent: &Agent<'_>, evaluation: Evaluation) -> Outcome {
        let stopped_by = self.guarded.stopped_by.locked().clone();
        let outcome = match (evaluation.outcome, &stopped_by) {
            (Ok(_), Some(tool)) => Err(format!("Stopped: the user refused {tool} at a permission prompt.")),
            (outcome, _) => outcome,
        };
        let report = Report {
            fresh: self.fresh,
            logs: evaluation.logs,
            refused: std::mem::take(&mut *self.refused.locked()),
            outcome,
            stopped_by,
            carried: std::mem::take(&mut *self.guarded.carried.locked()),
        };
        self.reported(agent, &report)
    }
}

#[derive(Debug)]
struct Guarded {
    stop: Arc<AtomicBool>,
    stopped_by: Mutex<Option<String>>,
    carried: Mutex<Vec<String>>,
    images: Mutex<Vec<domain::Image>>,
}

impl Guarded {
    fn settle(&self, call: HostCall, answer: impl FnOnce(&str, &Value) -> Result<Value, String>) {
        let HostCall { function, input, reply } = call;
        if let Some(source) = domain::untrusted::mcp_source(&function) {
            self.carried.locked().push(source);
        }
        let answered = answer(&function, &input);
        match self.stopped_by.locked().as_ref() {
            Some(tool) => reply.send(Err(format!("the user refused {tool} at a permission prompt; the script stops"))),
            None => reply.send(answered),
        }
    }

    fn watch_prompts(&self, event: &EngineEvent, asking: &Mutex<Vec<(domain::AskId, String)>>) {
        match event {
            EngineEvent::Asked(domain::AgentAsk { id, kind: domain::AskKind::Permission { tool, .. } }) => {
                asking.locked().push((id.clone(), tool.clone()));
            }
            EngineEvent::Answered { ask, decision: domain::Decision::Deny | domain::Decision::DenyWith(_) } => {
                let tool = asking.locked().iter().find(|(id, _)| id == ask).map(|(_, tool)| tool.clone());
                if let Some(tool) = tool {
                    self.stopped_by.locked().get_or_insert(tool);
                    self.stop.store(true, Ordering::Relaxed);
                }
            }
            _ => {}
        }
    }
}

impl ScriptRun {
    fn reported(&self, agent: &Agent<'_>, report: &Report) -> Outcome {
        if let Some(&index) = self.lane.get() {
            let status = match &report.outcome {
                Ok(_) => {
                    AgentStatus::Done { seconds: u32::try_from(self.started.elapsed().as_secs()).unwrap_or(u32::MAX) }
                }
                Err(_) if agent.control.is_stopped() || self.guarded.stop.load(Ordering::Relaxed) => {
                    AgentStatus::Cancelled
                }
                Err(_) => AgentStatus::Failed,
            };
            (agent.emit)(EngineEvent::AgentUpdate { index, status, summary: None });
        }
        let text = domain::code_mode::reported(report, domain::code_mode::PART);
        let mut outcome = if report.outcome.is_ok() { Outcome::ok(text) } else { Outcome::error(text) };
        outcome.images = std::mem::take(&mut *self.guarded.images.locked());
        outcome.carried_from(domain::code_mode::carried_from(report))
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

fn answered(block: Block, tool: &str, input: &Value) -> Result<Value, String> {
    match block {
        Block::ToolResult { content, is_error: false, .. } | Block::ToolResultRich { content, is_error: false, .. } => {
            Ok(domain::code_mode::script_value(tool, input, content))
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
