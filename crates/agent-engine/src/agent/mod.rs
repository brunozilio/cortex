mod call;
mod conversations;
pub mod current;
mod dispatch;
mod incoming;
mod model_hooks;
mod permission;
mod permission_ask;
mod plan_mode;
mod reminders;
mod script_call;
mod sub_agent;
mod tasks;
mod verify;
mod worktree;

use domain::EngineEvent;
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
                for task in tasks.background.lock().expect("tasks").values() {
                    task.control.cancel();
                }
            }
            outcome
        })
    }

    #[allow(clippy::needless_lifetimes)]
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
        let mut hook_fell_back = false;
        let mut stop_hook_active = false;
        let (prompt_at, mut attempts) = (history.len().saturating_sub(1), 0);
        for _ in 0..self.turn_limit.min(MAX_TURNS) {
            if self.control.is_stopped() {
                return Err(Ending::Cancelled);
            }
            let model = &self.workspace.model;
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
                let compaction = crate::upkeep::compact_hooked(self.api, &template, history, self.workspace, "auto")
                    .map_err(Ending::Failed)?;
                counted.forget();
                reminders.resend();
                (self.emit)(EngineEvent::Compacted(compaction));
                compacted_at = Some(history.len());
            }
            reminders.deliver(self.runtime_reminder(), history);
            let mut request = TurnRequest { messages: history.clone(), ..self.turn_template(fell_back.as_ref()) };
            if let Some(model) = self.hooked_model("ModelRequest", &request, serde_json::json!({})) {
                request.model = model;
            }
            let mut on = |delta: Delta| match delta {
                Delta::Text(text) => (self.emit)(EngineEvent::TextDelta(text)),
                Delta::Thinking(text) => (self.emit)(EngineEvent::ThinkingDelta(text)),
                Delta::ToolInput { name, piece } => (self.emit)(EngineEvent::ToolInputDelta { name, piece }),
            };
            let reply = match self.api.turn(&request, &mut on, self.control.stop_flag()) {
                Ok(reply) => reply,
                Err(ModelError::Cancelled) => return Err(Ending::Cancelled),
                Err(ModelError::Overloaded(_))
                    if fell_back.is_none()
                        && let Some(fallback) =
                            self.workspace.fallback.as_ref().filter(|model| **model != self.request.model) =>
                {
                    fell_back = Some(fallback.clone());
                    (self.emit)(EngineEvent::FellBack(fallback.clone()));
                    continue;
                }
                Err(ModelError::TooLong(_)) if recoveries < 2 => {
                    recoveries += 1;
                    let template = self.turn_template(fell_back.as_ref());
                    let compaction =
                        crate::upkeep::compact_hooked(self.api, &template, history, self.workspace, "auto")
                            .map_err(Ending::Failed)?;
                    counted.forget();
                    reminders.resend();
                    (self.emit)(EngineEvent::Compacted(compaction));
                    compacted_at = Some(history.len());
                    continue;
                }
                Err(error)
                    if !hook_fell_back
                        && let Some(model) = self.hooked_model(
                            "ModelError",
                            &request,
                            serde_json::json!({"error": error.to_string()}),
                        ) =>
                {
                    hook_fell_back = true;
                    (self.emit)(EngineEvent::FellBack(model.clone()));
                    fell_back = Some(model);
                    continue;
                }
                Err(error) => return Err(Ending::Failed(error)),
            };
            usage.add(reply.usage);
            let calls: Vec<(String, String, Value)> = reply
                .message
                .tool_uses()
                .map(|(id, name, input)| (id.to_owned(), name.to_owned(), input.clone()))
                .collect();
            if let Some(safety) = &self.workspace.safety {
                safety.observe(history);
            }
            history.push(reply.message);
            counted.observe(reply.usage, history.len());
            match reply.stop {
                StopReason::MaxTokens if calls.is_empty() => {
                    history.push(Message::user("Your output reached its token limit. Continue exactly at the cutoff without repeating prior text. New text is appended to the same response, so preserve code fences and machine-readable syntax. Finish the requested work."));
                    continue;
                }
                StopReason::Refusal => return Err(Ending::Refused),
                _ if calls.is_empty() => {
                    let steered = self.steered();
                    if !steered.is_empty() && !self.control.is_stopped() {
                        history.push(Message { role: Role::User, blocks: steered });
                        continue;
                    }
                    let pending: Vec<_> = tasks.background.lock().expect("tasks").values().cloned().collect();
                    let mut reports = Vec::new();
                    for task in pending {
                        if !task.collected.load(Ordering::Relaxed) {
                            reports.push(task.output(true).content);
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
                        history.push(Message::user("The configured response contract requires calling StructuredOutput with a valid result before completing. Use the tool now."));
                        continue;
                    }
                    let event = if self.depth > 0 { "SubagentStop" } else { "Stop" };
                    let hook = self.workspace.hooks.run_with_model(
                        event, &self.workspace.agent_name,
                        json!({"last_assistant_message":history.last().map(Message::text),"stop_hook_active":stop_hook_active}),
                        self.workspace,Some((self.api,&self.request)),
                    ).map_err(|e|Ending::Failed(ModelError::Provider(e)))?;
                    if let Some(reason) = hook.blocked {
                        stop_hook_active = true;
                        history.push(Message::user(format!("Completion blocked by hook: {reason}\n{}", hook.context)));
                        continue;
                    }
                    if let Some(feedback) = self.judged(&history[prompt_at..], &mut attempts)? {
                        history.push(feedback);
                        continue;
                    }
                    return Ok(usage);
                }
                _ => {}
            }
            let new_rules = self.pending_rules(&calls)?;
            let mut results: Vec<Block> = self.results(&calls, &new_rules, tasks, history, spawn);
            for ((_, name, input), result) in calls.iter().zip(&results) {
                if name == "StructuredOutput" && matches!(result, Block::ToolResult { is_error: false, .. }) {
                    structured = true;
                    (self.emit)(EngineEvent::StructuredOutput(input["result"].clone()));
                }
            }
            results.extend(self.steered());
            results.extend(self.notices());
            history.push(Message { role: Role::User, blocks: results });
            if self.control.is_moving() {
                return Err(Ending::Moved);
            }
        }
        Err(Ending::TooManyTurns)
    }

    fn runtime_reminder(&self) -> String {
        let style = self.workspace.prompt_context.as_ref().and_then(|context| context.output_style.as_ref());
        let reminder =
            crate::prompt::reminder(self.control.mode(), self.depth, style.and_then(|s| s.reminder.as_deref()));
        format!("{reminder}{}", self.workspace.rule_instructions())
    }

    fn turn_template(&self, fell_back: Option<&String>) -> TurnRequest {
        TurnRequest {
            messages: Vec::new(),
            system: self.request.system.clone(),
            tools: self.offered(),
            model: fell_back.cloned().unwrap_or_else(|| self.request.model.clone()),
            ..self.request.clone()
        }
    }

    pub(super) fn offered(&self) -> Vec<crate::model::ToolSpec> {
        let mut tools = self.request.tools.clone();
        if let Some(deferred) = &self.workspace.deferred {
            tools.extend(deferred.loaded());
        }
        tools
    }
}
