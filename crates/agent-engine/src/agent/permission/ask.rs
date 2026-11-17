use domain::permission_rules;
use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent};
use serde_json::Value;
use std::sync::mpsc::RecvError;

use super::Asking;
use crate::agent::Agent;
use crate::tools::{Access, Outcome, ask};

impl Agent<'_> {
    pub(in crate::agent) fn ask_user(&self, id: &str, input: &Value) -> Outcome {
        if self.control.no_prompts() {
            return Outcome::error("Interactive questions are disabled for this agent.");
        }
        let asked = match ask::questions(input) {
            Ok(asked) => asked,
            Err(problem) => return Outcome::error(format!("The questions could not be asked: {problem}.")),
        };
        self.notify("elicitation_dialog", concat!(brand::name!(), " has questions for you"));
        match self.asked(id, AskKind::Questions(asked.clone())) {
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
            Err(_) => Outcome::error(crate::tools::stopped::while_asking("the question")),
        }
    }

    pub(super) fn ask_permission(&self, asking: &Asking<'_>, shown: &str) -> Outcome {
        let Asking { id, tool, name, input, access, subject, .. } = *asking;
        let rule = permission_rules::suggest(name, subject, access == Access::ReadsOutside);
        self.notify(
            "permission_prompt",
            &format!("{brand_name} needs your permission to use {name}", brand_name = brand::NAME),
        );
        let kind = AskKind::Permission {
            tool: name.to_owned(),
            subject: shown.to_owned(),
            rememberable: self.always_asks(name, input).is_none(),
            rule: rule.clone(),
        };
        match self.asked(id, kind) {
            Ok(Decision::Allow | Decision::Answer(_) | Decision::Approve(_)) => self.went_ahead(id, tool, input),
            Ok(Decision::AllowAlways) => {
                self.remember_allowed(asking, rule);
                self.went_ahead(id, tool, input)
            }
            Ok(Decision::Deny) => self.refused(
                name,
                format!("The user did not allow {name} here. Do not retry it; ask them or take another approach."),
            ),
            Ok(Decision::DenyWith(note)) => {
                self.refused(name, format!("The user did not allow {name} here and said what to do instead: {note}"))
            }
            Err(_) => Outcome::error(crate::tools::stopped::while_asking(name)),
        }
    }

    fn remember_allowed(&self, asking: &Asking<'_>, rule: Option<String>) {
        match rule {
            Some(rule) => {
                match crate::prompt::permissions::keep_allow(&self.workspace.cwd, &rule) {
                    Ok(()) => self.say_when_the_rule_waits_for_trust(&rule),
                    Err(error) => tracing::warn!(%error, %rule, "the rule lasts this session only"),
                }
                self.control.remember(|remembered| remembered.allow_rule(&rule));
            }
            None => self.control.remember(|remembered| remembered.remember(asking.name, asking.access, asking.subject)),
        }
    }

    fn say_when_the_rule_waits_for_trust(&self, rule: &str) {
        let workspace = self.workspace;
        let kept = workspace.cwd.join(brand::DIR).join("settings.local.json");
        let gate = crate::trust::Gate::at(&workspace.cwd, workspace.home.as_deref(), workspace.policy.trust);
        if !gate.honours(&kept) {
            (self.emit)(EngineEvent::Notice(format!(
                "Kept {rule} in {}/settings.local.json. This folder is not trusted, so the rule holds for this run \
                 only; it applies to later ones once you trust the folder (/trust).",
                brand::DIR
            )));
        }
    }

    pub(in crate::agent) fn asked(&self, call: &str, kind: AskKind) -> Result<Decision, RecvError> {
        let ask = AskId(format!("ask-{}-{call}", uuid::Uuid::new_v4()));
        let answer = self.control.open(ask.clone());
        (self.emit)(EngineEvent::Asked(AgentAsk { id: ask.clone(), kind }));
        let answer = answer.recv();
        if let Ok(decision) = &answer {
            (self.emit)(EngineEvent::Answered { ask, decision: decision.clone() });
        }
        answer
    }

    pub(in crate::agent) fn notify(&self, kind: &str, message: &str) {
        let input = serde_json::json!({ "message": message, "notification_type": kind });
        if let Err(error) = self.workspace.hooks.run("Notification", kind, input, self.workspace) {
            tracing::warn!(%error, "a Notification hook failed");
        }
    }
}
