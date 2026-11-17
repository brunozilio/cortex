mod call;
mod command_class;
mod conversations;
pub mod current;
mod cutoffs;
mod dispatch;
mod egress;
mod incoming;
mod interception;
mod model_hooks;
mod permission;
mod permission_ask;
mod plan_mode;
mod reminders;
mod script_call;
mod stuck;
mod sub_agent;
mod sub_agent_model;
mod sub_agent_report;
mod tasks;
mod template;
mod this_prompt;
mod watch;
mod worktree;

use domain::EngineEvent;
use platform::sync::Locked;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

use self::tasks::{StopTasksOnDrop, Tasks};
use crate::control::Control;
use crate::model::{Delta, ModelApi, ModelError, ToolSpec, TurnRequest};
use crate::tools::subagent::FoundAgent;
use crate::tools::{Tool, Workspace};
use crate::transcript::{Block, Message, Role, StopReason, Usage};

pub const MAX_TURNS: usize = 200;
const SHOWN_OUTPUT: usize = 64_000;

#[derive(Debug, PartialEq)]
pub enum Ending {
    Cancelled,
    Failed(ModelError),
    Refused,
    TooManyTurns,
    Moved,
}

pub trait Review: Sync {
    fn review(&self, agent: &Agent<'_>, run: &[Message], attempts: &mut u8) -> Result<Option<Message>, Ending>;
}

pub struct Agent<'a> {
    pub api: &'a dyn ModelApi,
    pub tools: &'a [Box<dyn Tool>],
    pub workspace: &'a Workspace,
    pub control: &'a Control,
    pub emit: &'a (dyn Fn(EngineEvent) + Sync),
    pub request: TurnRequest,
    pub agents: &'a [FoundAgent],
    pub depth: usize,
    pub turn_limit: usize,
}

impl std::fmt::Debug for Agent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent").field("model", &self.request.model).finish_non_exhaustive()
    }
}

impl Agent<'_> {
    pub fn specs(tools: &[Box<dyn Tool>]) -> Vec<ToolSpec> {
        tools.iter().map(|tool| tool.spec()).collect()
    }

    pub fn run(&self, history: &mut Vec<Message>) -> Result<Usage, Ending> {
        let tasks = Tasks::default();
        std::thread::scope(|scope| {
            let _stop_tasks = StopTasksOnDrop(&tasks);
            let spawn = |job| {
                scope.spawn(job);
            };
            let outcome = self.run_loop(history, &tasks, &spawn);
            if outcome.is_err() {
                for task in tasks.background.locked().values() {
                    task.control.cancel();
                }
            }
            outcome
        })
    }

    #[allow(clippy::needless_lifetimes)]
    #[allow(clippy::too_many_lines, reason = "split planned: the turn loop becomes typed phases in a later phase")]
    fn run_loop<'env>(
        &'env self,
        history: &mut Vec<Message>,
        tasks: &Tasks,
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Result<Usage, Ending> {
        let mut usage = Usage::default();
        let mut structured = false;
        let mut recoveries = 0;
        let mut compacted_at = None;
        let mut counted = crate::upkeep::Counted::default();
        let mut reminders = reminders::Reminders::default();
        let mut fell_back: Option<String> = None;
        let mut fallback = domain::fallback::Fallback::new(self.workspace.models.fallbacks.clone());
        let mut rejections = domain::interception::Rejections::default();
        let mut stop_hook_active = false;
        let mut cutoffs = cutoffs::Cutoffs::default();
        let (mut prompt, mut attempts) = (this_prompt::ThisPrompt::last_of(history), 0);
        for _ in 0..self.turn_limit.min(MAX_TURNS) {
            if self.control.is_stopped() {
                return Err(Ending::Cancelled);
            }
            let model = &self.workspace.models.model;
            let measured = crate::upkeep::measure(model, &self.request.system, &self.request.tools, history);
            let threshold = model
                .capabilities
                .as_ref()
                .and_then(|c| c.auto_compact_token_limit)
                .unwrap_or_else(|| measured.window.saturating_sub(measured.reserved));
            let mut used = counted.used(measured.used, history);
            if used >= threshold && crate::upkeep::clear_old_results(history) > 0 {
                counted.forget();
                self.workspace.forget_seen_files();
                used = crate::upkeep::measure(model, &self.request.system, &self.request.tools, history).used;
            }
            if history.len() > 2 && used >= threshold && compacted_at != Some(history.len()) {
                let template = self.turn_template(fell_back.as_ref());
                let compacted = crate::upkeep::compact_hooked(self.api, &template, history, self.workspace, "auto");
                prompt.compacted(history);
                match compacted {
                    Ok(compaction) => {
                        counted.forget();
                        reminders.resend();
                        (self.emit)(EngineEvent::Compacted(compaction));
                        compacted_at = Some(history.len());
                    }
                    Err(ModelError::Cancelled) => return Err(Ending::Cancelled),
                    Err(error) => tracing::warn!(%error, "automatic compaction failed; the turn goes on without it"),
                }
            }
            reminders.deliver(self.runtime_reminder(used, threshold, reminders.quiet_tasks()), history);
            let mut request = TurnRequest { messages: history.clone(), ..self.turn_template(fell_back.as_ref()) };
            self.intercept_request(&mut request, &rejections)?;
            let holds_text = self.holds_text();
            let mut on = |delta: Delta| match delta {
                Delta::Text(_) if holds_text => {}
                Delta::Text(text) => (self.emit)(EngineEvent::TextDelta(text)),
                Delta::Thinking(text) => (self.emit)(EngineEvent::ThinkingDelta(text)),
                Delta::ToolInput { name, piece } => (self.emit)(EngineEvent::ToolInputDelta { name, piece }),
                Delta::Searching { id, query } => {
                    (self.emit)(EngineEvent::ToolStarted(domain::ToolCall::new(id, crate::tools::WEB_SEARCH, query)));
                }
                Delta::Searched { id, found } => {
                    (self.emit)(EngineEvent::ToolResult { id: id.clone(), output: found, failed: false });
                    (self.emit)(EngineEvent::ToolFinished { id });
                }
                Delta::HandedOver { from, to } => (self.emit)(EngineEvent::AccountSwitched { from, to }),
            };
            let reply = match self.api.turn(&request, &mut on, self.control.stop_flag()) {
                Ok(reply) => {
                    fallback.succeeded();
                    reply
                }
                Err(ModelError::Cancelled) => return Err(Ending::Cancelled),
                Err(ModelError::TooLong(_)) if recoveries < 2 => {
                    recoveries += 1;
                    let template = self.turn_template(fell_back.as_ref());
                    let compaction =
                        crate::upkeep::compact_hooked(self.api, &template, history, self.workspace, "auto")
                            .map_err(Ending::Failed)?;
                    prompt.compacted(history);
                    counted.forget();
                    reminders.resend();
                    (self.emit)(EngineEvent::Compacted(compaction));
                    compacted_at = Some(history.len());
                    continue;
                }
                Err(error) if let Some(model) = self.next_model(&mut fallback, &request, &error) => {
                    (self.emit)(EngineEvent::FellBack(model.clone()));
                    fell_back = Some(model);
                    continue;
                }
                Err(error) => return Err(Ending::Failed(error)),
            };
            usage.add(reply.usage);
            let reply = match self.intercept_reply(reply, &request, &mut rejections)? {
                interception::Intercepted::Accepted(reply) => reply,
                interception::Intercepted::Rejected => continue,
            };
            let calls: Vec<(String, String, Value)> = reply
                .message
                .tool_uses()
                .map(|(id, name, input)| (id.to_owned(), name.to_owned(), input.clone()))
                .collect();
            if cutoffs.ask_again_for_empty(&reply, !calls.is_empty()) {
                continue;
            }
            reminders.observe(&calls);
            if let Some(safety) = &self.workspace.policy.safety {
                safety.observe(history);
            }
            self.workspace.tracking.hear(egress::heard(history));
            history.push(reply.message);
            counted.observe(reply.usage, history.len());
            if reply.stop != StopReason::MaxTokens {
                cutoffs.answered();
            }
            match reply.stop {
                StopReason::MaxTokens if calls.is_empty() => {
                    let continuation = cutoffs.continuation(history.last().unwrap_or(&Message::user("")))?;
                    history.push(continuation);
                    continue;
                }
                StopReason::Refusal => return Err(Ending::Refused),
                StopReason::PauseTurn if calls.is_empty() => continue,
                _ if calls.is_empty() => {
                    let steered = self.steered();
                    if !steered.is_empty() && !self.control.is_stopped() {
                        history.push(Message { role: Role::User, blocks: steered });
                        continue;
                    }
                    let pending: Vec<_> =
                        tasks.background.locked().iter().map(|(id, task)| (id.clone(), task.clone())).collect();
                    let mut reports = Vec::new();
                    let saved = crate::tools::persisted::results_dir(&self.workspace.session_id);
                    for (id, task) in pending {
                        if !task.collected.load(Ordering::Relaxed) {
                            let report = task.output(true).content;
                            let short = crate::tools::persisted::persist(&report, &saved, &format!("report-{id}"));
                            reports.push(short.unwrap_or(report));
                        }
                    }
                    if !reports.is_empty() && !self.control.is_stopped() {
                        history.push(Message::user(format!(
                            "Background agents completed. Incorporate their reports into your answer:\n{}",
                            reports.join("\n\n")
                        )));
                        continue;
                    }
                    if self.control.is_stopped() {
                        return Err(Ending::Cancelled);
                    }
                    if self.tools.iter().any(|t| t.spec().name == "StructuredOutput") && !structured {
                        history.push(cutoffs.structured_nudge()?);
                        continue;
                    }
                    let event = if self.depth > 0 { "SubagentStop" } else { "Stop" };
                    let hook = self.workspace.hooks.run_with_model(
                        event, &self.workspace.agent_name,
                        json!({"last_assistant_message":history.last().map(Message::text),"stop_hook_active":stop_hook_active}),
                        self.workspace,Some((self.api,&self.request)),
                    ).map_err(|e|Ending::Failed(ModelError::Provider(e)))?;
                    match hook.blocked.map(|reason| cutoffs.stop_blocked(event, &reason, &hook.context)) {
                        Some(Ok(blocked)) => {
                            stop_hook_active = true;
                            history.push(blocked);
                            continue;
                        }
                        Some(Err(notice)) => (self.emit)(EngineEvent::TextDelta(notice)),
                        None => cutoffs.stop_allowed(),
                    }
                    if let Some(feedback) = self.reviewed(prompt.run(history), &mut attempts)? {
                        history.push(feedback);
                        continue;
                    }
                    return Ok(usage);
                }
                _ => {}
            }
            let new_rules = self.pending_rules(&calls);
            let mut results: Vec<Block> = self.results(&calls, &new_rules, tasks, history, spawn);
            for ((_, name, input), result) in calls.iter().zip(&results) {
                if name == "StructuredOutput" && matches!(result, Block::ToolResult { is_error: false, .. }) {
                    structured = true;
                    (self.emit)(EngineEvent::StructuredOutput(input["result"].clone()));
                }
            }
            if let Some(model) = self.help_when_stuck(&calls, &mut results, tasks) {
                fell_back = Some(model);
            }
            results.extend(self.steered());
            results.extend(self.notices());
            history.push(Message { role: Role::User, blocks: results });
            if let Some(why) = self.workspace.tracking.refusals_stop() {
                return Err(Ending::Failed(ModelError::Provider(why)));
            }
            if self.control.is_moving() {
                return Err(Ending::Moved);
            }
        }
        Err(Ending::TooManyTurns)
    }

    fn reviewed(&self, run: &[Message], attempts: &mut u8) -> Result<Option<Message>, Ending> {
        for reviewer in crate::tools::builtin::reviewers() {
            if let Some(feedback) = reviewer.review(self, run, attempts)? {
                return Ok(Some(feedback));
            }
        }
        Ok(None)
    }
}
