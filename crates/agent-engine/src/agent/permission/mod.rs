mod ask;
pub(in crate::agent) mod egress;
mod plan_mode;
mod policy;

use domain::EngineEvent;
use domain::permission::{Effect, Judgement, Reason, Standing, Verdict};
use domain::permission_rules::RuleKind;
use serde_json::{Value, json};

use super::Agent;
use crate::tools::{Outcome, Tool};

pub(in crate::agent) use policy::Called;

pub(in crate::agent) struct Asking<'r> {
    pub(in crate::agent) id: &'r str,
    pub(in crate::agent) tool: &'r dyn Tool,
    pub(in crate::agent) called: &'r Called<'r>,
    pub(in crate::agent) judgement: &'r Judgement,
    shown: Option<&'r str>,
}

impl Asking<'_> {
    fn shown(&self, reason: &Reason) -> String {
        let about = match &self.judgement.guarded {
            Some((path, guard)) => format!("{}\n{}", guard.why(), path.display()),
            None => self.shown.unwrap_or(&self.judgement.subject).to_owned(),
        };
        reason.heading().map_or(about.clone(), |heading| format!("{heading}\n{about}"))
    }
}

impl Agent<'_> {
    fn unattended(&self, standing: Standing) -> bool {
        self.control.mode() == domain::PermissionMode::Bypass && standing != Standing::Reads
    }

    pub(in crate::agent) fn permitted(
        &self,
        id: &str,
        tool: &dyn Tool,
        subject: &str,
        input: &Value,
        hook_permission: Option<RuleKind>,
    ) -> Outcome {
        self.permitted_as(id, tool, subject, None, input, hook_permission)
    }

    pub(in crate::agent) fn permitted_as(
        &self,
        id: &str,
        tool: &dyn Tool,
        subject: &str,
        shown: Option<&str>,
        input: &Value,
        hook_permission: Option<RuleKind>,
    ) -> Outcome {
        let name = tool.spec().name;
        let effects = tool.effects(input, self.workspace);
        let called = Called { name: &name, subject, input, effects: &effects };
        let judgement = self.judged(&called);
        let asking = Asking { id, tool, called: &called, judgement: &judgement, shown };
        self.decided(&asking, hook_permission)
    }

    fn decided(&self, asking: &Asking<'_>, hook_permission: Option<RuleKind>) -> Outcome {
        let original = asking.judgement.verdict.clone();
        if original.asked_every_time() {
            return self.carried_out(asking, original);
        }
        let rule_asks = original.asked_by_a_rule();
        let verdict = after_the_tool_hook(original, hook_permission, rule_asks, asking.judgement);
        let pre_approved = self.pre_approved(asking);
        if pre_approved && !rule_asks && !verdict.denies() && hook_permission != Some(RuleKind::Ask) {
            return self.went_ahead(asking.id, asking.tool, asking.called.input);
        }
        match self.after_the_permission_hook(asking, verdict, rule_asks) {
            Ok(verdict) => self.carried_out(asking, verdict),
            Err(settled) => settled,
        }
    }

    fn after_the_permission_hook(
        &self,
        asking: &Asking<'_>,
        verdict: Verdict,
        rule_asks: bool,
    ) -> Result<Verdict, Outcome> {
        if !verdict.asks() {
            return Ok(verdict);
        }
        let Called { name, input, .. } = *asking.called;
        let hook = self
            .workspace
            .hooks
            .run_with_model(
                crate::hooks::HookEvent::PermissionRequest,
                name,
                json!({"tool_name":name,"tool_input":input,"tool_use_id":asking.id}),
                self.workspace,
                Some((self.api, &self.request)),
            )
            .map_err(Outcome::error)?;
        if let Some(reason) = hook.blocked {
            return Err(self.refused(name, reason));
        }
        if let Some(updated) = hook.updated_input
            && updated != *input
        {
            return Err(Outcome::error(format!(
                "Permission hook updated input. Reissue the tool call with these arguments so path rules are checked: {updated}"
            )));
        }
        Ok(match hook.permission {
            Some(RuleKind::Allow) if !rule_asks => Verdict::Allow,
            _ => verdict,
        })
    }

    fn carried_out(&self, asking: &Asking<'_>, verdict: Verdict) -> Outcome {
        let Asking { id, tool, called, judgement, .. } = *asking;
        let Called { name, input, .. } = *called;
        let verdict = if self.control.no_prompts() { verdict.without_the_user(name) } else { verdict };
        match verdict {
            Verdict::Allow if self.unattended(judgement.standing) => {
                let instructions = self.workspace.instructions_text();
                let checked = self.workspace.policy.safety.as_deref().map_or(Ok(()), |safety| {
                    safety.check(self.api, &self.request, &instructions, name, input, self.control.stop_flag())
                });
                match checked {
                    Ok(()) => self.went_ahead(id, tool, input),
                    Err(reason) => self.refused(name, reason),
                }
            }
            Verdict::Allow => self.went_ahead(id, tool, input),
            Verdict::Deny(reason) => self.refused(name, reason),
            Verdict::Ask { reason, rememberable, rule } => {
                self.ask_permission(asking, &asking.shown(&reason), rememberable, rule)
            }
        }
    }

    fn pre_approved(&self, asking: &Asking<'_>) -> bool {
        let rules = &self.workspace.policy.pre_approved;
        let ruling = if asking.judgement.guarded.is_some() {
            self.ruling_in(&rules.naming_their_target(), asking.called)
        } else {
            self.ruling_in(rules, asking.called)
        };
        matches!(ruling, Some((RuleKind::Allow, _)))
    }

    pub(in crate::agent) fn went_ahead(&self, id: &str, tool: &dyn Tool, input: &Value) -> Outcome {
        self.workspace.tracking.count_refusal(false);
        if !tool.effects(input, self.workspace).iter().any(Effect::acts) {
            return tool.run(input, self.workspace);
        }
        let show = |line: String| {
            let line = self.workspace.policy.redacted(&line);
            (self.emit)(EngineEvent::ToolProgress { id: id.to_owned(), line });
        };
        crate::tools::progress::watched(&show, || tool.run(input, self.workspace))
    }

    pub(in crate::agent) fn refused(&self, tool: &str, reason: impl Into<String>) -> Outcome {
        self.workspace.tracking.count_refusal(true);
        let reason = reason.into();
        (self.emit)(EngineEvent::Refused { tool: tool.to_owned(), reason: reason.clone() });
        Outcome::error(reason)
    }
}

fn after_the_tool_hook(
    original: Verdict,
    hook_permission: Option<RuleKind>,
    rule_asks: bool,
    judgement: &Judgement,
) -> Verdict {
    match (original, hook_permission) {
        (Verdict::Deny(reason), _) => Verdict::Deny(reason),
        (Verdict::Allow, Some(RuleKind::Ask)) => judgement.asking(Reason::Hook),
        (Verdict::Ask { reason, rule, .. }, Some(RuleKind::Ask)) => Verdict::Ask { reason, rememberable: false, rule },
        (_, Some(RuleKind::Deny)) => Verdict::Deny("Hook denied the tool call".into()),
        (verdict, Some(RuleKind::Allow)) if rule_asks => verdict,
        (_, Some(RuleKind::Allow)) => Verdict::Allow,
        (verdict, None) => verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judgement() -> Judgement {
        Judgement {
            verdict: Verdict::Allow,
            standing: Standing::Acts,
            subject: "make".into(),
            guarded: None,
            rule: Some("Bash(make)".into()),
        }
    }

    #[test]
    fn a_tool_hook_s_answer_never_lifts_a_denial_nor_a_rule_that_asks() {
        let judgement = judgement();
        let after = |original, hook, rule_asks| after_the_tool_hook(original, hook, rule_asks, &judgement);
        let denied = || Verdict::Deny("no".into());
        let asked = || judgement.asking(Reason::Mode);
        let by_rule = || judgement.asking(Reason::Rule("Bash(make)".into()));
        for hook in [None, Some(RuleKind::Allow), Some(RuleKind::Ask), Some(RuleKind::Deny)] {
            assert_eq!(after(denied(), hook, false), denied(), "{hook:?}");
        }
        assert_eq!(after(asked(), Some(RuleKind::Allow), false), Verdict::Allow);
        assert_eq!(after(by_rule(), Some(RuleKind::Allow), true), by_rule(), "the rule still asks");
        assert_eq!(after(Verdict::Allow, Some(RuleKind::Ask), false), judgement.asking(Reason::Hook));
        assert!(
            matches!(
                after(asked(), Some(RuleKind::Ask), false),
                Verdict::Ask { reason: Reason::Mode, rememberable: false, .. }
            ),
            "the hook asks again next time, so always is not offered"
        );
        assert!(after(Verdict::Allow, Some(RuleKind::Deny), false).denies());
        assert_eq!(after(Verdict::Allow, None, false), Verdict::Allow);
        assert_eq!(after(by_rule(), None, true), by_rule());
    }
}
