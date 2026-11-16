use serde_json::{Value, json};

use super::Decision;
use super::brief::BRIEF;

const QUIET_FAILURES: [&str; 4] = [BRIEF, "ModelRequest", "ModelResponse", "ModelError"];

pub fn failed(event: &str, error: &str, decision: &mut Decision) {
    if QUIET_FAILURES.contains(&event) {
        tracing::warn!(event, error, "a hook failed and was skipped");
        return;
    }
    decision.context.push_str(&format!("Hook failed: {error}\n"));
}

pub fn absorb(event: &str, value: &Value, decision: &mut Decision, input: &mut Value) {
    let specific = &value["hookSpecificOutput"];
    match event {
        BRIEF => decision.brief = Some(value.clone()),
        "UserPromptSubmit" => {
            if let Some(prompt) = specific["updatedPrompt"].as_str() {
                decision.prompt = Some(prompt.to_owned());
                input["prompt"] = json!(prompt);
            }
        }
        "ModelRequest" => {
            if let Some(system) = specific["system"].as_str() {
                decision.system = Some(system.to_owned());
                input["system"] = json!(system);
            }
            if let Some(history) = specific.get("history") {
                decision.history = Some(history.clone());
                input["history"] = history.clone();
            }
        }
        _ => {}
    }
}
