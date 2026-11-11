//! The Copilot API's `GET /models`: the models this account may pick, as
//! the Copilot CLI's picker filters them.
use domain::model::ModelCapabilities;
use domain::{ModelOption, ModelProvider};
use serde_json::Value;

/// One catalog entry, or `None` for one the picker hides: a custom model,
/// a model neither offered in the picker nor enabled by policy, or one
/// that does not chat (embeddings).
pub fn model(item: &Value) -> Option<ModelOption> {
    let id = item["id"].as_str().map(str::trim).filter(|id| !id.is_empty())?;
    let picked = item["model_picker_enabled"] != false || item["policy"]["state"] == "enabled";
    let chats = item["capabilities"]["type"].as_str().is_none_or(|kind| kind == "chat");
    if item["custom_model"] == true || !picked || !chats {
        return None;
    }
    let caps = &item["capabilities"];
    let limits = &caps["limits"];
    let supports = &caps["supports"];
    let label = item["name"].as_str().map(str::trim).filter(|name| !name.is_empty()).unwrap_or(id);
    let provider = ModelProvider::CopilotSubscription;
    let mut model = ModelOption::new(format!("{}:{id}", provider.id()), label).served_by(provider);
    model.window = limits["max_context_window_tokens"].as_u64().filter(|n| *n > 0);
    let levels: Vec<String> = supports["reasoning_effort"]
        .as_array()
        .map(|levels| levels.iter().filter_map(|l| l.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    model.capabilities = Some(ModelCapabilities {
        reasoning: !levels.is_empty()
            || supports["reasoning_effort"] == true
            || supports["thinking"] == true
            || supports["max_thinking_budget"].as_u64().is_some_and(|n| n > 0),
        adaptive_thinking: supports["adaptive_thinking"] == true,
        max_output_tokens: limits["max_output_tokens"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0),
        reasoning_levels: levels,
        endpoints: item["supported_endpoints"]
            .as_array()
            .map(|paths| paths.iter().filter_map(|p| p.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
        ..ModelCapabilities::default()
    });
    Some(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_entry_gives_its_window_limits_effort_and_apis() {
        let entry = json!({"id": "gpt-5", "name": "GPT-5", "model_picker_enabled": true,
            "capabilities": {"type": "chat", "limits": {"max_context_window_tokens": 400000, "max_output_tokens": 128000},
                "supports": {"tool_calls": true, "reasoning_effort": ["low", "medium", "high"]}},
            "supported_endpoints": ["/chat/completions", "/responses"]});
        let found = model(&entry).unwrap_or_else(|| panic!("listed"));
        assert_eq!((found.id.as_str(), found.label.as_str()), ("copilot-subscription:gpt-5", "GPT-5"));
        assert_eq!(found.window, Some(400000));
        let caps = found.capabilities.unwrap_or_default();
        assert!(caps.reasoning);
        assert_eq!((caps.max_output_tokens, caps.reasoning_levels.len()), (Some(128000), 3));
        assert_eq!(caps.endpoints, ["/chat/completions", "/responses"]);
    }

    #[test]
    fn the_picker_hides_custom_disabled_and_non_chat_models() {
        let hidden = [
            json!({"id": "c", "custom_model": true}),
            json!({"id": "d", "model_picker_enabled": false, "policy": {"state": "unconfigured"}}),
            json!({"id": "e", "capabilities": {"type": "embeddings"}}),
            json!({"id": ""}),
        ];
        assert!(hidden.iter().all(|entry| model(entry).is_none()));
        let enabled = json!({"id": "o", "model_picker_enabled": false, "policy": {"state": "enabled"}});
        assert_eq!(model(&enabled).map(|m| m.label), Some("o".into()));
    }
}
