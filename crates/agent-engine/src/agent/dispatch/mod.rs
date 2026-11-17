mod after;
mod batches;
mod in_loop;
mod task_list;
mod watch;
mod worktree;

use domain::{EngineEvent, ToolCall};
use platform::sync::Locked;
use serde_json::{Value, json};

use super::Agent;
use super::run::ThisRun;
use super::subagents::background::Spawn;
use crate::hooks::{HookEvent, HookOutcome};
use crate::tools::{Outcome, Tool};
use crate::transcript::{Block, Message};
use domain::permission_rules::PermissionDecision;

pub(in crate::agent) struct Calling<'c, 'env> {
    pub(in crate::agent) id: &'c str,
    pub(in crate::agent) name: &'c str,
    pub(in crate::agent) tool: Option<&'c dyn Tool>,
    pub(in crate::agent) parent: &'c [Message],
    pub(in crate::agent) run: &'c ThisRun,
    pub(in crate::agent) spawn: Option<Spawn<'c, 'env>>,
    pub(in crate::agent) permission: Option<PermissionDecision>,
}

impl Agent<'_> {
    pub(in crate::agent) fn call<'env>(
        &'env self,
        id: &str,
        name: &str,
        input: &Value,
        run: &ThisRun,
        parent: &[Message],
        spawn: Spawn<'_, 'env>,
    ) -> Block {
        self.call_from(id, name, input, run, parent, Some(spawn))
    }

    fn call_from<'env>(
        &'env self,
        id: &str,
        name: &str,
        input: &Value,
        run: &ThisRun,
        parent: &[Message],
        spawn: Option<Spawn<'_, 'env>>,
    ) -> Block {
        let tool = self.tool(run, name);
        let offered = self.offers(name) || self.loads_on_call(name);
        let pre = self.before(id, name, input, tool.is_some() && offered);
        let input = pre.updated_input.as_ref().unwrap_or(input);
        let rule_context = self.rules_for_changed_input(&pre, input);
        self.started(id, name, tool, input);
        let calling = Calling { id, name, tool, parent, run, spawn, permission: pre.permission };
        let mut outcome = match self.refused_before_running(&calling, input, &pre, &rule_context, offered) {
            Some(refusal) => refusal,
            None => self.ran(&calling, input),
        };
        self.after(&calling, input, &pre.context, &mut outcome);
        self.finished(id, outcome)
    }

    pub(in crate::agent) fn offers(&self, name: &str) -> bool {
        self.request.tools.iter().any(|spec| spec.name == name)
            || self.workspace.services.deferred.as_ref().is_some_and(|deferred| deferred.is_loaded(name))
    }

    fn loads_on_call(&self, name: &str) -> bool {
        self.workspace.policy.load_on_call
            && self.workspace.services.deferred.as_ref().is_some_and(|deferred| deferred.load(name))
    }

    fn before(&self, id: &str, name: &str, input: &Value, known: bool) -> HookOutcome {
        if !known || self.control.is_stopped() {
            return HookOutcome::default();
        }
        self.workspace
            .hooks
            .run_with_model(
                HookEvent::PreToolUse,
                name,
                json!({"tool_name":name,"tool_use_id":id,"tool_input":input}),
                self.workspace,
                Some((self.api, &self.request)),
            )
            .unwrap_or_else(|error| HookOutcome { blocked: Some(error), ..Default::default() })
    }

    fn rules_for_changed_input(&self, pre: &HookOutcome, input: &Value) -> Vec<String> {
        let mut rule_context = Vec::new();
        if pre.updated_input.is_none() {
            return rule_context;
        }
        let path = input["file_path"].as_str().or_else(|| input["path"].as_str()).map(|p| self.workspace.resolve(p));
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
        rule_context
    }

    fn started(&self, id: &str, name: &str, tool: Option<&dyn Tool>, input: &Value) {
        let summary = tool.map(|tool| tool.summary(input)).unwrap_or_default();
        let shown_summary = self.workspace.policy.redacted(&summary);
        let shown_input = redacted_value(input, &|text| self.workspace.policy.redacted(text));
        (self.emit)(EngineEvent::ToolStarted(ToolCall::new(id, name, shown_summary).with_input(shown_input)));
    }

    fn refused_before_running(
        &self,
        calling: &Calling<'_, '_>,
        input: &Value,
        pre: &HookOutcome,
        rule_context: &[String],
        offered: bool,
    ) -> Option<Outcome> {
        let Calling { name, tool, run, .. } = *calling;
        if let Some(reason) = &pre.blocked {
            return Some(Outcome::error(format!("Hook blocked {name}: {reason}")));
        }
        if !rule_context.is_empty() {
            return Some(Outcome::error(format!(
                "Hook changed input. Load these project instructions and reissue the tool call: {}",
                rule_context.join("\n")
            )));
        }
        if tool.is_some() && !offered {
            return Some(Outcome::error(format!(
                "Tool {name} is not available to this agent (load it with ToolSearch if listed there)."
            )));
        }
        if let Some(refusal) = tool.and_then(|tool| self.invalid_input(name, tool, input)) {
            return Some(Outcome::error(refusal));
        }
        if self.control.is_stopped() {
            return Some(Outcome::error(crate::tools::stopped::before_start()));
        }
        if let Some(tool) = tool
            && !run.admit(name, input, !crate::tools::changes_nothing(tool, input, self.workspace))
        {
            (self.emit)(EngineEvent::Repeated { tool: name.to_owned() });
            run.stuck.held_back();
            return Some(Outcome::error(domain::repeats::refused(name)));
        }
        let watching = run.watches.still_running(self.workspace.credentials.client);
        domain::watch::while_watching(name, &watching).map(Outcome::error)
    }

    pub(in crate::agent) fn invalid_input(&self, name: &str, tool: &dyn Tool, input: &Value) -> Option<String> {
        match self.request.tools.iter().find(|spec| spec.name == name) {
            Some(offered) => crate::tools::checked::refusal(name, &offered.schema, input),
            None => crate::tools::checked::refusal(name, &tool.spec().schema, input),
        }
    }

    fn ran<'env>(&'env self, calling: &Calling<'_, 'env>, input: &Value) -> Outcome {
        let declared =
            calling.tool.is_some_and(|tool| tool.script().is_none()) && crate::tools::declares_execute(calling.name);
        if declared {
            self.execute_declared(calling, input, &|input| self.in_loop(calling, input))
        } else {
            self.in_loop(calling, input)
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

#[cfg(test)]
mod tests;
