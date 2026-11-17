use domain::{EngineEvent, ToolCall};
use platform::sync::Locked;
use serde_json::{Value, json};

use super::tasks::Tasks;
use super::{Agent, SHOWN_OUTPUT};
use crate::tools::ask::ASK_TOOL;
use crate::tools::plan_mode::{ENTER_PLAN_TOOL, EXIT_PLAN_TOOL};
use crate::tools::send_message::SEND_MESSAGE_TOOL;
use crate::tools::subagent::{AGENT_TOOL, AgentTool};
use crate::tools::worktree::{ENTER_WORKTREE, EXIT_WORKTREE};
use crate::tools::{Outcome, clip};
use crate::transcript::{Block, Message};

impl Agent<'_> {
    #[allow(clippy::needless_lifetimes)]
    #[allow(clippy::too_many_lines, reason = "split planned: one match over every tool, to become a dispatch per tool")]
    pub(super) fn call<'env>(
        &'env self,
        id: &str,
        name: &str,
        input: &Value,
        tasks: &Tasks,
        parent: &[Message],
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Block {
        let tool = self.tools.iter().find(|tool| tool.spec().name == name);
        let offered = self.request.tools.iter().any(|spec| spec.name == name)
            || self.workspace.services.deferred.as_ref().is_some_and(|deferred| deferred.is_loaded(name));
        let pre = if tool.is_some() && offered && !self.control.is_stopped() {
            self.workspace.hooks.run_with_model(
                "PreToolUse",
                name,
                json!({"tool_name":name,"tool_use_id":id,"tool_input":input}),
                self.workspace,
                Some((self.api, &self.request)),
            )
        } else {
            Ok(Default::default())
        };
        let pre = pre.unwrap_or_else(|error| crate::hooks::Decision { blocked: Some(error), ..Default::default() });
        let input = pre.updated_input.as_ref().unwrap_or(input);
        let mut rule_context = Vec::new();
        if pre.updated_input.is_some() {
            let path =
                input["file_path"].as_str().or_else(|| input["path"].as_str()).map(|p| self.workspace.resolve(p));
            let mut seen = self.workspace.tracking.seen_rules.locked();
            for rule in &self.workspace.rules {
                match rule.applies(&self.workspace.cwd, path.as_deref()) {
                    Ok(true) if !rule.patterns.is_empty() && seen.insert(rule.path.clone()) => {
                        rule_context.push(rule.text.clone())
                    }
                    Err(error) => rule_context.push(error),
                    _ => {}
                }
            }
        }
        let summary = tool.map(|tool| tool.summary(input)).unwrap_or_default();
        let shown_summary = self.workspace.policy.redacted(&summary);
        let shown_input = redacted_value(input, &|text| self.workspace.policy.redacted(text));
        (self.emit)(EngineEvent::ToolStarted(ToolCall::new(id, name, shown_summary).with_input(shown_input)));
        let native = |input: &Value| -> Outcome {
            let summary = tool.map(|tool| tool.summary(input)).unwrap_or_default();
            if name == "TaskOutput" {
                tasks.output(input)
            } else if name == "TaskStop" {
                tasks.stop(input)
            } else if name == "TaskList" {
                tasks.list(|| {
                    tool.map(|tool| tool.run(input, self.workspace))
                        .and_then(|out| serde_json::from_str::<Value>(&out.content).ok())
                        .unwrap_or_else(|| json!([]))
                })
            } else if let Some(outcome) = tool.and_then(|tool| {
                let agent = crate::tools::Consult {
                    api: self.api,
                    request: &self.request,
                    conversation: parent,
                    stop: self.control.stop_flag(),
                };
                tool.consult(input, self.workspace, &agent)
            }) {
                outcome
            } else {
                let permission = pre.permission.as_deref();
                match tool {
                    None => Outcome::error(format!("There is no tool named {name}.")),
                    Some(_) if name == ASK_TOOL => self.ask_user(id, input),
                    Some(_) if name == EXIT_PLAN_TOOL => self.exit_plan_mode(id, input),
                    Some(_) if name == ENTER_PLAN_TOOL => self.enter_plan_mode(),
                    Some(_) if name == ENTER_WORKTREE => self.enter_worktree(tasks),
                    Some(_) if name == EXIT_WORKTREE => self.exit_worktree(input, tasks),
                    Some(_) if name == AGENT_TOOL && (AgentTool { agents: self.agents.to_vec() }).background(input) => {
                        self.start_background(input, tasks, parent, spawn)
                    }
                    Some(_) if name == AGENT_TOOL => {
                        self.sub_agent_with(input, self.control, self.workspace, parent, &tasks.conversations, None)
                    }
                    Some(_) if name == SEND_MESSAGE_TOOL && self.to_session(input, &tasks.conversations) => {
                        self.permitted(id, &crate::tools::peers::SessionMessage, &summary, input, permission)
                    }
                    Some(_) if name == SEND_MESSAGE_TOOL => self.send_message(input, &tasks.conversations),
                    Some(_) if name == crate::tools::watch::WATCH_TOOL => self.watch(id, input, tasks),
                    Some(_) if name == crate::tools::repl::REPL_TOOL && crate::tools::repl::in_background(input) => {
                        self.repl_in_background(input, tasks, spawn)
                    }
                    Some(_) if name == crate::tools::repl::REPL_TOOL => self.repl(id, input, tasks, parent),
                    Some(tool) => match (self.project_skill(tool.as_ref(), input), tool.script()) {
                        (Some(skill), _) => self.skill_call(id, tool.as_ref(), skill, &summary, input, permission),
                        (None, Some(script)) => self.script_call(id, script, &summary, input, parent, permission),
                        (None, None) => self.permitted(id, tool.as_ref(), &summary, input, permission),
                    },
                }
            }
        };
        let mut outcome = if let Some(reason) = &pre.blocked {
            Outcome::error(format!("Hook blocked {name}: {reason}"))
        } else if !rule_context.is_empty() {
            Outcome::error(format!(
                "Hook changed input. Load these project instructions and reissue the tool call: {}",
                rule_context.join("\n")
            ))
        } else if tool.is_some() && !offered {
            Outcome::error(format!(
                "Tool {name} is not available to this agent (load it with ToolSearch if listed there)."
            ))
        } else if self.control.is_stopped() {
            Outcome::error(crate::tools::stopped::before_start())
        } else if tool.is_some()
            && !tasks.admit(name, input, tool.is_some_and(|tool| tool.access() != crate::tools::Access::Reads))
        {
            (self.emit)(EngineEvent::Repeated { tool: name.to_owned() });
            if tasks.stuck.locked().held_back() {
                tasks.help_due.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Outcome::error(domain::repeats::refused(name))
        } else if let Some(refusal) =
            domain::watch::while_watching(name, &tasks.still_watching(self.workspace.credentials.client))
        {
            Outcome::error(refusal)
        } else if tool.is_some_and(|tool| tool.script().is_none()) && crate::tools::declares_execute(name) {
            self.execute_declared(id, name, input, parent, &native)
        } else {
            native(input)
        };
        if !pre.context.is_empty() {
            outcome.content.push_str(&format!("\nHook context: {}", pre.context));
        }
        let event = if outcome.is_error { "PostToolUseFailure" } else { "PostToolUse" };
        match self.workspace.hooks.run_with_model(
            event,
            name,
            json!({"tool_name":name,"tool_use_id":id,"tool_input":input,"tool_response":outcome.content}),
            self.workspace,
            Some((self.api, &self.request)),
        ) {
            Ok(hook) => {
                if let Some(output) = hook.updated_output {
                    outcome.content = output.as_str().map(str::to_owned).unwrap_or_else(|| output.to_string());
                    outcome.images.clear();
                }
                if let Some(reason) = hook.blocked {
                    outcome.is_error = true;
                    outcome.content.push_str(&format!("\nHook feedback: {reason}"));
                }
                outcome.content.push_str(&hook.context);
            }
            Err(error) => {
                outcome.is_error = true;
                outcome.content.push_str(&format!("\nHook failed: {error}"));
            }
        }
        outcome.content = self.workspace.policy.redacted(&domain::untrusted::neutralize(&outcome.content));
        outcome.shown = outcome.shown.map(|shown| self.workspace.policy.redacted(&shown));
        if name == "WebFetch" && !outcome.is_error {
            let (url, looking_for) = (crate::tools::field(input, "url"), crate::tools::field(input, "prompt"));
            let guarded = crate::tools::builtin::web::guarded::reader(
                self.workspace.models.small_model.as_deref(),
                &self.request,
                self.workspace.models.model.provider,
            );
            let read = if self.workspace.policy.web.small_pages_direct {
                crate::tools::builtin::web::guarded::digest
            } else {
                crate::tools::builtin::web::guarded::read_apart
            };
            outcome.content = read(
                self.api,
                &guarded,
                &self.request.model,
                url.unwrap_or_default(),
                looking_for.unwrap_or_default(),
                &outcome.content,
                self.control.stop_flag(),
            );
        }
        if name != "Read" {
            let dir = crate::tools::persisted::results_dir(&self.workspace.session_id);
            if let Some(short) = crate::tools::persisted::persist(&outcome.content, &dir, id) {
                outcome.content = short;
            }
        }
        if matches!(name, "TaskCreate" | "TaskUpdate") && !outcome.is_error {
            self.emit_tasks();
        }
        if let Some(edited) = outcome.edited.take() {
            (self.emit)(EngineEvent::Edited(*edited));
        }
        (self.emit)(EngineEvent::ToolFinished { id: id.to_owned() });
        (self.emit)(EngineEvent::ToolResult {
            id: id.to_owned(),
            output: clip(outcome.shown.as_deref().unwrap_or(&outcome.content), SHOWN_OUTPUT),
            failed: outcome.is_error,
        });
        if outcome.images.is_empty() {
            Block::ToolResult { id: id.to_owned(), content: outcome.content, is_error: outcome.is_error }
        } else {
            Block::ToolResultRich {
                id: id.to_owned(),
                content: outcome.content,
                images: outcome.images,
                is_error: outcome.is_error,
            }
        }
    }
}

fn redacted_value(value: &Value, redact: &dyn Fn(&str) -> String) -> Value {
    match value {
        Value::String(text) => Value::String(redact(text)),
        Value::Array(items) => Value::Array(items.iter().map(|item| redacted_value(item, redact)).collect()),
        Value::Object(fields) => {
            Value::Object(fields.iter().map(|(key, item)| (key.clone(), redacted_value(item, redact))).collect())
        }
        other => other.clone(),
    }
}
