mod handlers;
mod model;
mod script_hook;
mod scripts;
mod validate;

pub use self::handlers::{command, http};
pub use self::script_hook::script;
use crate::tools::Workspace;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hooks(pub BTreeMap<String, Vec<Group>>);
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
    pub kind: String,
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
    #[serde(default)]
    pub status_message: Option<String>,
}
fn default_timeout() -> u64 {
    600
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Decision {
    pub blocked: Option<String>,
    pub context: String,
    pub updated_input: Option<Value>,
    pub permission: Option<String>,
    pub updated_output: Option<Value>,
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<domain::Effort>,
    #[serde(default)]
    pub text: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
impl Hooks {
    pub fn listed(&self) -> Vec<domain::permission_rules::HookInfo> {
        let mut listed = Vec::new();
        for (event, groups) in &self.0 {
            for group in groups {
                for handler in &group.hooks {
                    let runs = match handler.kind.as_str() {
                        "http" => handler.url.clone().unwrap_or_default(),
                        "prompt" | "agent" => {
                            let prompt: String =
                                handler.prompt.as_deref().unwrap_or_default().chars().take(60).collect();
                            format!("{}: {prompt}", handler.kind)
                        }
                        _ => handler.command.clone(),
                    };
                    listed.push(domain::permission_rules::HookInfo {
                        event: event.clone(),
                        matcher: group.matcher.clone(),
                        runs,
                    });
                }
            }
        }
        listed
    }

    pub fn parse(value: &Value) -> Result<Self, String> {
        let events: BTreeMap<String, Vec<Group>> = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        for (event, groups) in &events {
            validate::event(event)?;
            for group in groups {
                validate::matcher(event, &group.matcher)?;
            }
            for (at, hook) in groups.iter().flat_map(|group| &group.hooks).enumerate() {
                validate::hook(event, at, hook)?;
            }
        }
        Ok(Self(events))
    }
    pub fn extend(&mut self, other: &Self) {
        for (event, groups) in &other.0 {
            self.0.entry(event.clone()).or_default().extend(groups.clone());
        }
    }
    pub fn run(&self, event: &str, subject: &str, input: Value, workspace: &Workspace) -> Result<Decision, String> {
        self.run_with_model(event, subject, input, workspace, None)
    }
    pub fn run_with_model(
        &self,
        event: &str,
        subject: &str,
        mut input: Value,
        workspace: &Workspace,
        model: Option<(&dyn crate::model::ModelApi, &crate::model::TurnRequest)>,
    ) -> Result<Decision, String> {
        let mut decision = Decision::default();
        input["hook_event_name"] = json!(event);
        input["session_id"] = json!(workspace.session_id);
        input["agent_type"] = json!(workspace.agent_name);
        input["cwd"] = json!(workspace.cwd);
        for group in self.0.get(event).into_iter().flatten() {
            if !group.matcher.is_empty()
                && group.matcher != "*"
                && !regex::Regex::new(&group.matcher).map_err(|e| e.to_string())?.is_match(subject)
            {
                continue;
            }
            for handler in &group.hooks {
                let result: CommandOutput = if matches!(handler.kind.as_str(), "command" | "http" | "script") {
                    match handler.kind.as_str() {
                        "http" => http(handler, &input, workspace)?,
                        "script" => script(handler, &input, workspace, model)?,
                        _ => command(handler, &input, workspace)?,
                    }
                } else {
                    let (api, template) = model.ok_or("model hook requires an active model context")?;
                    let verdict = model::evaluate(handler, &input, workspace, api, template)?;
                    CommandOutput {
                        code: Some(0),
                        stdout: if verdict["ok"] == true {
                            "{}".into()
                        } else {
                            json!({"decision":"block","reason":verdict["reason"].as_str().unwrap_or("Model hook rejected the action")}).to_string()
                        },
                        stderr: String::new(),
                    }
                };
                if result.code == Some(2) {
                    decision.blocked = Some(result.stderr);
                    return Ok(decision);
                }
                if result.code != Some(0) {
                    decision.context.push_str(&format!("Hook failed: {}\n", result.stderr));
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(&result.stdout) {
                    let specific = &value["hookSpecificOutput"];
                    if value["continue"] == false
                        || value["decision"] == "block"
                        || specific["permissionDecision"] == "deny"
                    {
                        decision.blocked = Some(
                            value["reason"]
                                .as_str()
                                .or_else(|| value["stopReason"].as_str())
                                .or_else(|| specific["permissionDecisionReason"].as_str())
                                .unwrap_or("Blocked by configured hook")
                                .into(),
                        );
                    }
                    let permission = if event == "PermissionRequest" {
                        specific["decision"]["behavior"].as_str()
                    } else {
                        specific["permissionDecision"].as_str()
                    };
                    if let Some(permission) = permission {
                        if !matches!(permission, "allow" | "deny" | "ask") {
                            return Err("invalid permission decision from hook".into());
                        }
                        decision.permission = Some(permission.into());
                        if permission == "deny" {
                            decision.blocked = Some(
                                specific["permissionDecisionReason"]
                                    .as_str()
                                    .unwrap_or("Permission hook denied the action")
                                    .into(),
                            );
                        }
                    }
                    if event == "PostToolUse" {
                        decision.updated_output = specific.get("updatedToolOutput").cloned();
                    }
                    if decision.model.is_none() && matches!(event, "ModelRequest" | "ModelError") {
                        decision.model = specific["model"].as_str().filter(|m| !m.trim().is_empty()).map(Into::into);
                    }
                    if decision.effort.is_none() && event == "ModelRequest" {
                        decision.effort = specific["effort"].as_str().and_then(domain::interception::effort);
                    }
                    if decision.text.is_none() && event == "ModelResponse" {
                        decision.text = specific["text"].as_str().map(Into::into);
                    }
                    for text in
                        [value["systemMessage"].as_str(), specific["additionalContext"].as_str()].into_iter().flatten()
                    {
                        decision.context.push_str(text);
                        decision.context.push('\n');
                    }
                    if matches!(event, "PreToolUse" | "PermissionRequest")
                        && let Some(updated) = if event == "PermissionRequest" {
                            specific["decision"].get("updatedInput")
                        } else {
                            specific.get("updatedInput")
                        }
                    {
                        if !updated.is_object() {
                            return Err("hook updatedInput must be an object".into());
                        }
                        decision.updated_input = Some(updated.clone());
                        input["tool_input"] = updated.clone();
                    }
                } else if matches!(event, "SessionStart" | "UserPromptSubmit") {
                    decision.context.push_str(&result.stdout);
                }
                if decision.blocked.is_some() {
                    return Ok(decision);
                }
            }
        }
        Ok(decision)
    }
}
