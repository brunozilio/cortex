mod brief;
mod handlers;
mod model;
mod rewrites;
mod script_hook;
mod scripts;
mod validate;

pub use self::brief::Briefed;
pub use self::handlers::{HookRunError, command, http};
pub use self::script_hook::script;
pub use self::scripts::BUILTIN as BUILTIN_SCRIPT;
pub(crate) use self::validate::TIMEOUT_SECONDS;
use crate::tools::Workspace;
pub use domain::hooks::{HandlerKind, HookEvent};
use domain::permission_rules::PermissionDecision;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hooks(pub BTreeMap<HookEvent, Vec<Group>>);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    #[serde(default)]
    pub matcher: String,
    pub hooks: Vec<Handler>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handler {
    #[serde(rename = "type")]
    pub kind: HandlerKind,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub allowed_env_vars: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default, skip_serializing, deserialize_with = "accepted_and_dropped")]
    pub status_message: (),
}
fn default_timeout() -> u64 {
    600
}
fn accepted_and_dropped<'de, D: serde::Deserializer<'de>>(written: D) -> Result<(), D::Error> {
    serde::de::IgnoredAny::deserialize(written).map(|_| ())
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HookOutcome {
    pub blocked: Option<String>,
    pub context: String,
    pub updated_input: Option<Value>,
    pub permission: Option<PermissionDecision>,
    pub updated_output: Option<Value>,
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<domain::Effort>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub brief: Option<Value>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub history: Option<Value>,
    #[serde(default)]
    pub prompt: Option<String>,
}
impl HookOutcome {
    fn without_trailing_whitespace(mut self) -> Self {
        self.blocked = self.blocked.map(|reason| reason.trim_end().to_owned());
        self.context.truncate(self.context.trim_end().len());
        self
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
type Model<'a> = Option<(&'a dyn crate::model::ModelApi, &'a crate::model::TurnRequest)>;

#[derive(Debug, PartialEq, Eq)]
enum Handled {
    GoOn,
    Blocked,
}

impl Group {
    fn applies_to(&self, subject: &str) -> Result<bool, String> {
        if self.matcher.is_empty() || self.matcher == "*" {
            return Ok(true);
        }
        Ok(regex::Regex::new(&self.matcher).map_err(|e| e.to_string())?.is_match(subject))
    }
}

impl Handler {
    fn runs(&self, input: &Value, workspace: &Workspace, model: Model<'_>) -> Result<CommandOutput, String> {
        match self.kind {
            HandlerKind::Http => http(self, input, workspace).map_err(String::from),
            HandlerKind::Script => script(self, input, workspace, model),
            HandlerKind::Command => command(self, input, workspace).map_err(String::from),
            HandlerKind::Prompt | HandlerKind::Agent => self.judged(input, workspace, model),
        }
    }

    fn judged(&self, input: &Value, workspace: &Workspace, model: Model<'_>) -> Result<CommandOutput, String> {
        let (api, template) = model.ok_or("model hook requires an active model context")?;
        let verdict = model::evaluate(self, input, workspace, api, template)?;
        let stdout = if verdict["ok"] == true {
            "{}".into()
        } else {
            let reason = verdict["reason"].as_str().unwrap_or("Model hook rejected the action");
            json!({"decision":"block","reason":reason}).to_string()
        };
        Ok(CommandOutput { code: Some(0), stdout, stderr: String::new() })
    }
}

impl Hooks {
    pub fn listed(&self) -> Vec<domain::permission_rules::HookInfo> {
        let mut listed = Vec::new();
        for (event, groups) in &self.0 {
            for group in groups {
                for handler in &group.hooks {
                    let runs = match handler.kind {
                        HandlerKind::Http => handler.url.clone().unwrap_or_default(),
                        HandlerKind::Prompt | HandlerKind::Agent => {
                            let prompt: String =
                                handler.prompt.as_deref().unwrap_or_default().chars().take(60).collect();
                            format!("{}: {prompt}", handler.kind)
                        }
                        HandlerKind::Command | HandlerKind::Script => handler.command.clone(),
                    };
                    listed.push(domain::permission_rules::HookInfo {
                        event: *event,
                        matcher: group.matcher.clone(),
                        runs,
                    });
                }
            }
        }
        listed
    }

    pub fn parse(value: &Value) -> Result<Self, String> {
        validate::named(value)?;
        let events: BTreeMap<HookEvent, Vec<Group>> =
            serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        for (event, groups) in &events {
            for group in groups {
                validate::matcher(*event, &group.matcher)?;
            }
            for (at, hook) in groups.iter().flat_map(|group| &group.hooks).enumerate() {
                validate::hook(*event, at, hook)?;
            }
        }
        Ok(Self(events))
    }
    pub fn extend(&mut self, other: &Self) {
        for (event, groups) in &other.0 {
            self.0.entry(*event).or_default().extend(groups.clone());
        }
    }
    pub fn listens_to(&self, event: HookEvent) -> bool {
        self.0.contains_key(&event)
    }
    pub fn run(
        &self,
        event: HookEvent,
        subject: &str,
        input: Value,
        workspace: &Workspace,
    ) -> Result<HookOutcome, String> {
        self.run_with_model(event, subject, input, workspace, None)
    }
    pub fn run_with_model(
        &self,
        event: HookEvent,
        subject: &str,
        input: Value,
        workspace: &Workspace,
        model: Option<(&dyn crate::model::ModelApi, &crate::model::TurnRequest)>,
    ) -> Result<HookOutcome, String> {
        self.decided(event, subject, input, workspace, model).map(HookOutcome::without_trailing_whitespace)
    }
    fn decided(
        &self,
        event: HookEvent,
        subject: &str,
        mut input: Value,
        workspace: &Workspace,
        model: Model<'_>,
    ) -> Result<HookOutcome, String> {
        let mut decision = HookOutcome::default();
        input["hook_event_name"] = json!(event);
        input["session_id"] = json!(workspace.session_id);
        input["agent_type"] = json!(workspace.agent_name);
        input["cwd"] = json!(workspace.cwd);
        for group in self.0.get(&event).into_iter().flatten() {
            if !group.applies_to(subject)? {
                continue;
            }
            for handler in &group.hooks {
                let result = handler.runs(&input, workspace, model)?;
                if absorbed(event, result, &mut decision, &mut input)? == Handled::Blocked {
                    return Ok(decision);
                }
            }
        }
        Ok(decision)
    }
}

fn absorbed(
    event: HookEvent,
    result: CommandOutput,
    decision: &mut HookOutcome,
    input: &mut Value,
) -> Result<Handled, String> {
    if result.code == Some(2) {
        decision.blocked = Some(result.stderr);
        return Ok(Handled::Blocked);
    }
    if result.code != Some(0) {
        rewrites::failed(event, &result.stderr, decision);
        return Ok(Handled::GoOn);
    }
    match serde_json::from_str::<Value>(&result.stdout) {
        Ok(value) => answered(event, &value, decision, input)?,
        Err(_) if event.takes_plain_text_as_context() => {
            decision.context.push_str(&result.stdout);
        }
        Err(_) => {}
    }
    Ok(if decision.blocked.is_some() { Handled::Blocked } else { Handled::GoOn })
}

fn answered(event: HookEvent, value: &Value, decision: &mut HookOutcome, input: &mut Value) -> Result<(), String> {
    let specific = &value["hookSpecificOutput"];
    if let Some(reason) = blocking(value) {
        decision.blocked = Some(reason);
    }
    permission_answered(event, specific, decision)?;
    if event == HookEvent::PostToolUse {
        decision.updated_output = specific.get("updatedToolOutput").cloned();
    }
    if decision.model.is_none() && matches!(event, HookEvent::ModelRequest | HookEvent::ModelError) {
        decision.model = specific["model"].as_str().filter(|m| !m.trim().is_empty()).map(Into::into);
    }
    if decision.effort.is_none() && event == HookEvent::ModelRequest {
        decision.effort = specific["effort"].as_str().and_then(domain::interception::effort);
    }
    rewrites::absorb(event, value, decision, input);
    if decision.text.is_none() && event == HookEvent::ModelResponse {
        decision.text = specific["text"].as_str().map(Into::into);
    }
    for text in [value["systemMessage"].as_str(), specific["additionalContext"].as_str()].into_iter().flatten() {
        decision.context.push_str(text);
        decision.context.push('\n');
    }
    input_updated(event, specific, decision, input)
}

fn blocking(value: &Value) -> Option<String> {
    let specific = &value["hookSpecificOutput"];
    let blocks = value["continue"] == false || value["decision"] == "block" || specific["permissionDecision"] == "deny";
    blocks.then(|| {
        value["reason"]
            .as_str()
            .or_else(|| value["stopReason"].as_str())
            .or_else(|| specific["permissionDecisionReason"].as_str())
            .unwrap_or("Blocked by configured hook")
            .into()
    })
}

fn permission_answered(event: HookEvent, specific: &Value, decision: &mut HookOutcome) -> Result<(), String> {
    let permission = if event == HookEvent::PermissionRequest {
        specific["decision"]["behavior"].as_str()
    } else {
        specific["permissionDecision"].as_str()
    };
    let Some(permission) = permission else { return Ok(()) };
    let permission = PermissionDecision::named(permission).ok_or("invalid permission decision from hook")?;
    decision.permission = Some(permission);
    if permission == PermissionDecision::Deny {
        let reason = specific["permissionDecisionReason"].as_str().unwrap_or("Permission hook denied the action");
        decision.blocked = Some(reason.into());
    }
    Ok(())
}

fn input_updated(
    event: HookEvent,
    specific: &Value,
    decision: &mut HookOutcome,
    input: &mut Value,
) -> Result<(), String> {
    let updated = match event {
        HookEvent::PermissionRequest => specific["decision"].get("updatedInput"),
        HookEvent::PreToolUse => specific.get("updatedInput"),
        _ => None,
    };
    let Some(updated) = updated else { return Ok(()) };
    if !updated.is_object() {
        return Err("hook updatedInput must be an object".into());
    }
    decision.updated_input = Some(updated.clone());
    input["tool_input"] = updated.clone();
    Ok(())
}

#[cfg(test)]
mod tests;
