use domain::permission_rules::{self, RuleKind};
use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent};
use serde_json::Value;
use std::sync::mpsc::RecvError;

use super::Agent;
use crate::tools::{Access, Outcome, Tool};

type Ruling = Option<(RuleKind, String)>;

impl Agent<'_> {
    pub(super) fn ask_permission(
        &self,
        id: &str,
        tool: &dyn Tool,
        access: Access,
        subject: &str,
        shown: &str,
        input: &Value,
    ) -> Outcome {
        let name = tool.spec().name;
        let rule = permission_rules::suggest(&name, subject, access == Access::ReadsOutside);
        self.notify(
            "permission_prompt",
            &format!("{brand_name} needs your permission to use {name}", brand_name = brand::NAME),
        );
        let kind = AskKind::Permission {
            tool: name.clone(),
            subject: shown.to_owned(),
            rememberable: self.always_asks(&name, input).is_none(),
            rule: rule.clone(),
        };
        match self.asked(id, kind) {
            Ok(Decision::Allow | Decision::Answer(_) | Decision::Approve(_)) => self.went_ahead(id, tool, input),
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
                self.went_ahead(id, tool, input)
            }
            Ok(Decision::Deny) => self.refused(
                &name,
                format!("The user did not allow {name} here. Do not retry it; ask them or take another approach."),
            ),
            Ok(Decision::DenyWith(note)) => {
                self.refused(&name, format!("The user did not allow {name} here and said what to do instead: {note}"))
            }
            Err(_) => Outcome::error(crate::tools::stopped::while_asking(&name)),
        }
    }

    pub(super) fn asked(&self, call: &str, kind: AskKind) -> Result<Decision, RecvError> {
        let ask = AskId(format!("ask-{}-{call}", uuid::Uuid::new_v4()));
        let answer = self.control.open(ask.clone());
        (self.emit)(EngineEvent::Asked(AgentAsk { id: ask.clone(), kind }));
        let answer = answer.recv();
        if let Ok(decision) = &answer {
            (self.emit)(EngineEvent::Answered { ask, decision: decision.clone() });
        }
        answer
    }

    pub(super) fn notify(&self, kind: &str, message: &str) {
        let input = serde_json::json!({ "message": message, "notification_type": kind });
        if let Err(error) = self.workspace.hooks.run("Notification", kind, input, self.workspace) {
            tracing::warn!(%error, "a Notification hook failed");
        }
    }

    pub(super) fn guarded_write(&self, tool: &dyn Tool, input: &Value) -> Option<(String, String)> {
        let target = tool.writes_to(input, self.workspace)?;
        let guarded = self.workspace.guarded_write(&target)?;
        let path = guarded.path.display().to_string();
        let shown = format!("{}\n{path}", guarded.why);
        Some((path, shown))
    }

    pub(super) fn rule_for(&self, name: &str, subject: &str, input: &Value, guarded: bool) -> Ruling {
        let remembered = self.control.remembered();
        if guarded {
            let named = self.workspace.policy.permissions.naming_their_target();
            let ruling = self.workspace.ruling_with(&named, name, subject, input);
            let recalled = remembered.rules().naming_their_target();
            return ruling.or_else(|| self.workspace.ruling_with(&recalled, name, subject, input));
        }
        self.workspace
            .ruling(name, subject, input)
            .or_else(|| self.workspace.ruling_with(remembered.rules(), name, subject, input))
    }

    pub(super) fn pre_approved(&self, name: &str, subject: &str, input: &Value, guarded: bool) -> bool {
        let rules = &self.workspace.policy.pre_approved;
        let ruling = if guarded {
            self.workspace.ruling_with(&rules.naming_their_target(), name, subject, input)
        } else {
            self.workspace.ruling_with(rules, name, subject, input)
        };
        matches!(ruling, Some((RuleKind::Allow, _)))
    }
}
