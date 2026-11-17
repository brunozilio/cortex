use domain::EngineEvent;
use serde_json::{Value, json};

use super::Calling;
use crate::agent::{Agent, SHOWN_OUTPUT};
use crate::tools::after::{Ran, after, kept_whole};
use crate::tools::{Consult, Outcome, clip, names};
use crate::transcript::Block;

impl Agent<'_> {
    pub(super) fn after(&self, calling: &Calling<'_, '_>, input: &Value, hook_context: &str, outcome: &mut Outcome) {
        let raw = crate::tools::raw_text::active();
        if !hook_context.is_empty() && !raw {
            outcome.content.push_str(&format!("\nHook context: {hook_context}"));
        }
        self.after_hook(calling, input, raw, outcome);
        self.made_safe_to_read(raw, outcome);
        self.read_as_its_tool_asks(calling, input, outcome);
        if !kept_whole(calling.name) {
            let dir = crate::tools::persisted::results_dir(&self.workspace.session_id);
            if let Some(short) = crate::tools::persisted::persist(&outcome.content, &dir, calling.id) {
                outcome.content = short;
            }
        }
        if names::changes_the_task_list(calling.name) && !outcome.is_error {
            self.emit_tasks(calling.run);
        }
    }

    fn after_hook(&self, calling: &Calling<'_, '_>, input: &Value, raw: bool, outcome: &mut Outcome) {
        let (id, name) = (calling.id, calling.name);
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
                if !raw {
                    outcome.content.push_str(&hook.context);
                }
            }
            Err(error) => {
                outcome.is_error = true;
                outcome.content.push_str(&format!("\nHook failed: {error}"));
            }
        }
    }

    fn made_safe_to_read(&self, raw: bool, outcome: &mut Outcome) {
        let policy = &self.workspace.policy;
        if raw {
            outcome.shown = Some(policy.redacted(outcome.shown.as_deref().unwrap_or(&outcome.content)));
        } else {
            outcome.content = policy.redacted(&domain::untrusted::neutralize(&outcome.content));
        }
        outcome.shown = outcome.shown.take().map(|shown| policy.redacted(&shown));
    }

    fn read_as_its_tool_asks(&self, calling: &Calling<'_, '_>, input: &Value, outcome: &mut Outcome) {
        let Some(read) = after(calling.name).filter(|_| !outcome.is_error) else { return };
        let agent = Consult {
            api: self.api,
            request: &self.request,
            conversation: calling.parent,
            stop: self.control.stop_flag(),
        };
        outcome.content = read(&Ran { input, workspace: self.workspace, agent: &agent }, &outcome.content);
    }

    pub(super) fn finished(&self, id: &str, mut outcome: Outcome) -> Block {
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
