use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent};
use serde_json::{Value, json};

use super::Agent;
use crate::permission::{Verdict, ruled, verdict};
use crate::tools::ask;
use crate::tools::{Access, Outcome, Tool};

impl Agent<'_> {
    pub(super) fn ask_user(&self, id: &str, input: &Value) -> Outcome {
        if self.control.no_prompts() {
            return Outcome::error("Interactive questions are disabled for this agent.");
        }
        let asked = match ask::questions(input) {
            Ok(asked) => asked,
            Err(problem) => return Outcome::error(format!("The questions could not be asked: {problem}.")),
        };
        self.notify("elicitation_dialog", concat!(brand::name!(), " has questions for you"));
        let ask = AskId(format!("ask-{}-{id}", uuid::Uuid::new_v4()));
        let answer = self.control.open(ask.clone());
        (self.emit)(EngineEvent::Asked(AgentAsk { id: ask, kind: AskKind::Questions(asked.clone()) }));
        match answer.recv() {
            Ok(Decision::Answer(picks)) => Outcome::ok(ask::answers(&asked, &picks)),
            Ok(Decision::Allow | Decision::AllowAlways | Decision::Approve(_)) => {
                Outcome::ok(ask::answers(&asked, &[]))
            }
            Ok(Decision::Deny) => Outcome::error(
                "The user declined to answer. Do not ask again; go on with your best judgement and say what you assumed.",
            ),
            Ok(Decision::DenyWith(note)) => {
                Outcome::error(format!("The user declined to answer the questions and said instead: {note}"))
            }
            Err(_) => Outcome::error("The run was cancelled."),
        }
    }

    fn unattended(&self, access: Access) -> bool {
        self.control.mode() == domain::PermissionMode::Bypass && access != Access::Reads
    }

    pub(super) fn permitted(
        &self,
        id: &str,
        tool: &dyn Tool,
        subject: &str,
        input: &Value,
        hook_permission: Option<&str>,
    ) -> Outcome {
        let name = tool.spec().name;
        let outside = (tool.access() == Access::Reads)
            .then(|| tool.reaches_outside(input, self.workspace))
            .flatten()
            .map(|path| path.display().to_string());
        let (access, subject) = match &outside {
            Some(path) => (Access::ReadsOutside, path.as_str()),
            None => (tool.access(), subject),
        };
        let mode = self.control.mode();
        let by_mode = verdict(mode, access, &name, subject, &self.control.remembered());
        let rule = self.workspace.ruling(&name, subject, input).or_else(|| self.remembered_rule(&name, subject, input));
        let rule_asks = matches!(rule, Some((domain::permission_rules::RuleKind::Ask, _)));
        let original = ruled(mode, access, rule, by_mode);
        let rule_asks = rule_asks && original == Verdict::Ask;
        let verdict = match (original, hook_permission) {
            (Verdict::Deny(reason), _) => Verdict::Deny(reason),
            (_, Some("ask")) => Verdict::Ask,
            (_, Some("deny")) => Verdict::Deny("Hook denied the tool call".into()),
            (verdict, Some("allow")) if rule_asks => verdict,
            (_, Some("allow")) => Verdict::Allow,
            (verdict, _) => verdict,
        };
        let permission_hook = if matches!(verdict, Verdict::Ask) {
            match self.workspace.hooks.run_with_model(
                "PermissionRequest",
                &name,
                json!({"tool_name":name,"tool_input":input,"tool_use_id":id}),
                self.workspace,
                Some((self.api, &self.request)),
            ) {
                Ok(hook) => hook,
                Err(error) => return Outcome::error(error),
            }
        } else {
            Default::default()
        };
        if let Some(reason) = permission_hook.blocked {
            return Outcome::error(reason);
        }
        if let Some(updated) = permission_hook.updated_input
            && updated != *input
        {
            return Outcome::error(format!(
                "Permission hook updated input. Reissue the tool call with these arguments so path rules are checked: {updated}"
            ));
        }
        let verdict = match (verdict, permission_hook.permission.as_deref()) {
            (Verdict::Ask, Some("allow")) if !rule_asks => Verdict::Allow,
            (verdict, _) => verdict,
        };
        match verdict {
            Verdict::Allow if self.unattended(access) => {
                let instructions = self.workspace.instructions_text();
                let checked = self.workspace.safety.as_deref().map_or(Ok(()), |safety| {
                    safety.check(self.api, &self.request, &instructions, &name, input, self.control.stop_flag())
                });
                match checked {
                    Ok(()) => tool.run(input, self.workspace),
                    Err(reason) => Outcome::error(reason),
                }
            }
            Verdict::Allow => tool.run(input, self.workspace),
            Verdict::Deny(reason) => Outcome::error(reason),
            Verdict::Ask if self.control.no_prompts() => {
                Outcome::error(format!("{name} requires permission; dontAsk denies unapproved actions."))
            }
            Verdict::Ask => self.ask_permission(id, tool, access, subject, input),
        }
    }
}
