mod ask;
mod command_class;
pub(in crate::agent) mod egress;
mod plan_mode;

use domain::EngineEvent;
use domain::command_policy::CommandClass;
use domain::permission_rules::RuleKind;
use serde_json::{Value, json};

use super::Agent;
use crate::permission::{Verdict, ruled, verdict};
use crate::tools::{Access, Outcome, Tool, names};

type Ruling = Option<(RuleKind, String)>;

pub(in crate::agent) struct Asking<'r> {
    pub(in crate::agent) id: &'r str,
    pub(in crate::agent) tool: &'r dyn Tool,
    pub(in crate::agent) name: &'r str,
    pub(in crate::agent) input: &'r Value,
    pub(in crate::agent) access: Access,
    pub(in crate::agent) subject: &'r str,
    shown: Option<&'r str>,
    guarded: bool,
}

impl Asking<'_> {
    fn shown(&self) -> &str {
        self.shown.unwrap_or(self.subject)
    }
}

struct Concern {
    refusal: String,
    shown: String,
}

impl Agent<'_> {
    fn unattended(&self, access: Access) -> bool {
        self.control.mode() == domain::PermissionMode::Bypass && access != Access::Reads
    }

    pub(in crate::agent) fn permitted(
        &self,
        id: &str,
        tool: &dyn Tool,
        subject: &str,
        input: &Value,
        hook_permission: Option<&str>,
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
        hook_permission: Option<&str>,
    ) -> Outcome {
        let name = tool.spec().name;
        let outside = (tool.access() == Access::Reads)
            .then(|| tool.reaches_outside(input, self.workspace))
            .flatten()
            .map(|path| path.display().to_string());
        let class = self.command_class(&name, input);
        if let CommandClass::Forbidden(reason) = class {
            return self.refused(&name, reason);
        }
        let guarded = self.guarded_write(tool, input);
        let (access, subject) = match (&outside, &guarded) {
            (Some(path), _) => (Access::ReadsOutside, path.as_str()),
            (None, Some((path, _))) => (Access::EditsGuarded, path.as_str()),
            (None, None) if class == CommandClass::ReadOnly => (Access::Reads, subject),
            (None, None) => (tool.access(), subject),
        };
        let shown = guarded.as_ref().map(|(_, shown)| shown.as_str()).or(shown);
        let asking = Asking { id, tool, name: &name, input, access, subject, shown, guarded: guarded.is_some() };
        self.decided(&asking, hook_permission)
    }

    fn decided(&self, asking: &Asking<'_>, hook_permission: Option<&str>) -> Outcome {
        let Asking { name, input, access, subject, guarded, .. } = *asking;
        let mode = self.control.mode();
        let by_mode = verdict(mode, access, name, subject, &self.control.remembered());
        let rule = self.rule_for(name, subject, input, guarded);
        let rule_asks = matches!(rule, Some((RuleKind::Ask, _)));
        let original = ruled(mode, access, rule, by_mode);
        if !matches!(original, Verdict::Deny(_))
            && let Some(concern) = self.concern(asking)
        {
            return self.asked_about(asking, concern);
        }
        let rule_asks = rule_asks && original == Verdict::Ask;
        let verdict = after_the_tool_hook(original, hook_permission, rule_asks);
        let pre_approved = self.pre_approved(name, subject, input, guarded);
        if pre_approved && !rule_asks && !matches!(verdict, Verdict::Deny(_)) && hook_permission != Some("ask") {
            return self.went_ahead(asking.id, asking.tool, input);
        }
        match self.after_the_permission_hook(asking, verdict, rule_asks) {
            Ok(verdict) => self.carried_out(asking, verdict),
            Err(settled) => settled,
        }
    }

    fn concern(&self, asking: &Asking<'_>) -> Option<Concern> {
        let (name, input, shown) = (asking.name, asking.input, asking.shown());
        if let Some(why) = self.always_asks(name, input) {
            return Some(Concern {
                refusal: format!("Not run: {why} The user cannot be asked here."),
                shown: format!("{why}\n{shown}"),
            });
        }
        if name == names::BASH && self.leaves_the_sandbox(input) {
            return Some(Concern {
                refusal: "Not run: leaving the sandbox needs the user, who cannot be asked here.".into(),
                shown: format!("Run OUTSIDE the sandbox (it may write anywhere):\n{shown}"),
            });
        }
        let concern = self.package_concerns(name, input)?;
        Some(Concern {
            refusal: format!("Not run: {concern}. Ask the user before installing it."),
            shown: format!("Check the package first: {concern}\n{shown}"),
        })
    }

    fn asked_about(&self, asking: &Asking<'_>, concern: Concern) -> Outcome {
        if self.control.no_prompts() {
            return self.refused(asking.name, concern.refusal);
        }
        self.ask_permission(asking, &concern.shown)
    }

    fn after_the_permission_hook(
        &self,
        asking: &Asking<'_>,
        verdict: Verdict,
        rule_asks: bool,
    ) -> Result<Verdict, Outcome> {
        if !matches!(verdict, Verdict::Ask) {
            return Ok(verdict);
        }
        let (name, input) = (asking.name, asking.input);
        let hook = self
            .workspace
            .hooks
            .run_with_model(
                "PermissionRequest",
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
        Ok(match hook.permission.as_deref() {
            Some("allow") if !rule_asks => Verdict::Allow,
            _ => verdict,
        })
    }

    fn carried_out(&self, asking: &Asking<'_>, verdict: Verdict) -> Outcome {
        let Asking { id, tool, name, input, access, .. } = *asking;
        match verdict {
            Verdict::Allow if self.unattended(access) => {
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
            Verdict::Ask if self.control.no_prompts() => {
                self.refused(name, format!("{name} requires permission; dontAsk denies unapproved actions."))
            }
            Verdict::Ask => self.ask_permission(asking, asking.shown()),
        }
    }

    fn leaves_the_sandbox(&self, input: &Value) -> bool {
        self.workspace.sandboxed() && input["unsandboxed"].as_bool() == Some(true)
    }

    fn package_concerns(&self, name: &str, input: &Value) -> Option<String> {
        let network = self.workspace.services.network.as_ref().filter(|_| self.workspace.policy.dependency_guard)?;
        let command = input["command"].as_str().filter(|_| name == names::BASH)?;
        let found = crate::guard::packages::concerns(&*network.0, command, crate::guard::packages::today());
        (!found.is_empty()).then(|| found.join("; "))
    }

    fn guarded_write(&self, tool: &dyn Tool, input: &Value) -> Option<(String, String)> {
        let target = tool.writes_to(input, self.workspace)?;
        let guarded = self.workspace.guarded_write(&target)?;
        let path = guarded.path.display().to_string();
        let shown = format!("{}\n{path}", guarded.why);
        Some((path, shown))
    }

    pub(in crate::agent) fn rule_for(&self, name: &str, subject: &str, input: &Value, guarded: bool) -> Ruling {
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

    fn pre_approved(&self, name: &str, subject: &str, input: &Value, guarded: bool) -> bool {
        let rules = &self.workspace.policy.pre_approved;
        let ruling = if guarded {
            self.workspace.ruling_with(&rules.naming_their_target(), name, subject, input)
        } else {
            self.workspace.ruling_with(rules, name, subject, input)
        };
        matches!(ruling, Some((RuleKind::Allow, _)))
    }

    pub(in crate::agent) fn went_ahead(&self, id: &str, tool: &dyn Tool, input: &Value) -> Outcome {
        self.workspace.tracking.count_refusal(false);
        if tool.access() != crate::tools::Access::Executes {
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

fn after_the_tool_hook(original: Verdict, hook_permission: Option<&str>, rule_asks: bool) -> Verdict {
    match (original, hook_permission) {
        (Verdict::Deny(reason), _) => Verdict::Deny(reason),
        (_, Some("ask")) => Verdict::Ask,
        (_, Some("deny")) => Verdict::Deny("Hook denied the tool call".into()),
        (verdict, Some("allow")) if rule_asks => verdict,
        (_, Some("allow")) => Verdict::Allow,
        (verdict, _) => verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_hook_s_answer_never_lifts_a_denial_nor_a_rule_that_asks() {
        let denied = || Verdict::Deny("no".into());
        for hook in [None, Some("allow"), Some("ask"), Some("deny")] {
            assert_eq!(after_the_tool_hook(denied(), hook, false), denied(), "{hook:?}");
        }
        assert_eq!(after_the_tool_hook(Verdict::Ask, Some("allow"), false), Verdict::Allow);
        assert_eq!(after_the_tool_hook(Verdict::Ask, Some("allow"), true), Verdict::Ask, "the rule still asks");
        assert_eq!(after_the_tool_hook(Verdict::Allow, Some("ask"), false), Verdict::Ask);
        assert!(matches!(after_the_tool_hook(Verdict::Allow, Some("deny"), false), Verdict::Deny(_)));
        assert_eq!(after_the_tool_hook(Verdict::Allow, Some("unknown"), false), Verdict::Allow);
        assert_eq!(after_the_tool_hook(Verdict::Ask, None, true), Verdict::Ask);
    }
}
