use domain::permission_rules::{self, RuleKind};
use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent};
use serde_json::Value;

use super::Agent;
use crate::tools::{Access, Outcome, Tool};

impl Agent<'_> {
    pub(super) fn ask_permission(
        &self,
        id: &str,
        tool: &dyn Tool,
        access: Access,
        subject: &str,
        input: &Value,
    ) -> Outcome {
        let name = tool.spec().name;
        let rule = permission_rules::suggest(&name, subject, access == Access::ReadsOutside);
        self.notify(
            "permission_prompt",
            &format!("{brand_name} needs your permission to use {name}", brand_name = brand::NAME),
        );
        let ask = AskId(format!("ask-{}-{id}", uuid::Uuid::new_v4()));
        let answer = self.control.open(ask.clone());
        let kind = AskKind::Permission {
            tool: name.clone(),
            subject: subject.to_owned(),
            rememberable: true,
            rule: rule.clone(),
        };
        (self.emit)(EngineEvent::Asked(AgentAsk { id: ask, kind }));
        match answer.recv() {
            Ok(Decision::Allow | Decision::Answer(_) | Decision::Approve(_)) => self.went_ahead(tool, input),
            Ok(Decision::AllowAlways) => {
                match rule {
                    Some(rule) => {
                        if let Err(error) = crate::prompt::permissions::keep_allow(&self.workspace.cwd, &rule) {
                            tracing::warn!(%error, %rule, "the rule lasts this session only");
                        }
                        self.control.remember(|remembered| remembered.allow_rule(&rule));
                    }
                    None => self.control.remember(|remembered| remembered.remember(&name, access, subject)),
                }
                self.went_ahead(tool, input)
            }
            Ok(Decision::Deny) => self.refused(format!(
                "The user did not allow {name} here. Do not retry it; ask them or take another approach."
            )),
            Ok(Decision::DenyWith(note)) => {
                self.refused(format!("The user did not allow {name} here and said what to do instead: {note}"))
            }
            Err(_) => Outcome::error(crate::tools::stopped::while_asking(&name)),
        }
    }

    pub(super) fn notify(&self, kind: &str, message: &str) {
        let input = serde_json::json!({ "message": message, "notification_type": kind });
        if let Err(error) = self.workspace.hooks.run("Notification", kind, input, self.workspace) {
            tracing::warn!(%error, "a Notification hook failed");
        }
    }

    pub(super) fn remembered_rule(&self, name: &str, subject: &str, input: &Value) -> Option<(RuleKind, String)> {
        let remembered = self.control.remembered();
        self.workspace.ruling_with(remembered.rules(), name, subject, input)
    }
}
