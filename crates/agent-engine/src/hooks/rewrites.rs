use serde_json::{Value, json};

use super::{HookEvent, HookOutcome};

pub fn failed(event: HookEvent, error: &str, decision: &mut HookOutcome) {
    if event.fails_quietly() {
        tracing::warn!(event = event.as_str(), error, "a hook failed and was skipped");
        return;
    }
    decision.context.push_str(&format!("Hook failed: {error}\n"));
}

pub fn absorb(event: HookEvent, value: &Value, decision: &mut HookOutcome, input: &mut Value) {
    let specific = &value["hookSpecificOutput"];
    match event {
        HookEvent::Brief => decision.brief = Some(value.clone()),
        HookEvent::UserPromptSubmit => {
            if let Some(prompt) = specific["updatedPrompt"].as_str() {
                decision.prompt = Some(prompt.to_owned());
                input["prompt"] = json!(prompt);
            }
        }
        HookEvent::ModelRequest => {
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
