use crossbeam_channel::RecvError;
use domain::permission::Judgement;
use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent};
use serde_json::Value;

use super::{Asking, Called};
use crate::agent::Agent;
use crate::tools::{Outcome, ask};

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

    pub(super) fn ask_permission(
        &self,
        asking: &Asking<'_>,
        shown: &str,
        rememberable: bool,
        rule: Option<String>,
    ) -> Outcome {
        let Asking { id, tool, called, .. } = *asking;
        let Called { name, input, .. } = *called;
        self.notify(
            "permission_prompt",
            &format!("{brand_name} needs your permission to use {name}", brand_name = brand::NAME),
        );
        let kind =
            AskKind::Permission { tool: name.to_owned(), subject: shown.to_owned(), rememberable, rule: rule.clone() };
        match self.asked_relaying(id, kind, Some(input)) {
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
            None => {
                let Judgement { standing, subject, .. } = asking.judgement;
                self.control.remember(|remembered| remembered.remember(asking.called.name, *standing, subject));
            }
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
        self.asked_relaying(call, kind, None)
    }

    fn asked_relaying(&self, call: &str, kind: AskKind, input: Option<&Value>) -> Result<Decision, RecvError> {
        let ask = AskId(format!("ask-{}-{call}", uuid::Uuid::new_v4()));
        let mut answer = self.control.open(ask.clone());
        let relayed = match (&kind, input) {
            (AskKind::Permission { tool, subject, .. }, Some(input)) => Some((tool.clone(), subject.clone(), input)),
            _ => None,
        };
        (self.emit)(EngineEvent::Asked(AgentAsk { id: ask.clone(), kind }));
        let _relay = relayed.and_then(|(tool, subject, input)| {
            let client = self.workspace.credentials.client;
            crate::mcp::channel_relay::ask(client, call, &tool, &subject, input, self.control.answerer(&ask))
        });
        let answer = answer.recv();
        if let Ok(decision) = &answer {
            (self.emit)(EngineEvent::Answered { ask, decision: decision.clone() });
        }
        answer
    }

    pub(in crate::agent) fn notify(&self, kind: &str, message: &str) {
        let input = serde_json::json!({ "message": message, "notification_type": kind });
        if let Err(error) = self.workspace.hooks.run(crate::hooks::HookEvent::Notification, kind, input, self.workspace)
        {
            tracing::warn!(%error, "a Notification hook failed");
        }
    }
}
