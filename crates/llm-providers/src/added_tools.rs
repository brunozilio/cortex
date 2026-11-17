use serde_json::{Value, json};

pub fn openai(model: &str) -> bool {
    let Some(version) = model.rsplit(':').next().and_then(|name| name.strip_prefix("gpt-")) else { return false };
    let version = version.split('-').next().unwrap_or_default();
    let (major, minor) = version.split_once('.').unwrap_or((version, "0"));
    match (major.parse::<u32>(), minor.parse::<u32>()) {
        (Ok(major), Ok(minor)) => major > 5 || (major == 5 && minor >= 4),
        _ => false,
    }
}

pub fn into_tools(body: &mut Value) {
    let Some(input) = body.get_mut("input").and_then(Value::as_array_mut) else { return };
    let mut added = Vec::new();
    input.retain(|item| {
        let moved = item["type"] == "additional_tools";
        if moved {
            added.extend(item["tools"].as_array().cloned().unwrap_or_default());
        }
        !moved
    });
    if added.is_empty() {
        return;
    }
    if !body["tools"].is_array() {
        body["tools"] = json!([]);
    }
    if let Some(tools) = body["tools"].as_array_mut() {
        tools.extend(added);
    }
}

pub const TOOL_SEARCH: &str = "ToolSearch";
pub const LOADED: &str = "Loaded; call them from now on:\n";
pub const WAITING: &str = "\n\nNot loaded yet: ";

pub fn loaded_names(result: &str) -> Vec<String> {
    let Some(shown) = result.strip_prefix(LOADED) else { return Vec::new() };
    let shown = shown.split_once(WAITING).map_or(shown, |(shown, _)| shown);
    let shown: Vec<Value> = serde_json::from_str(shown).unwrap_or_default();
    shown.iter().filter_map(|tool| tool["name"].as_str().map(str::to_owned)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpt_5_4_and_later_take_tools_in_the_conversation() {
        for model in ["gpt-5.4", "gpt-5.4-mini", "gpt-5.6-sol", "gpt-6-astra", "openai-subscription:gpt-6.1"] {
            assert!(openai(model), "{model}");
        }
        for model in ["gpt-5", "gpt-5.3-codex", "gpt-4.1", "o3", "claude-x", "gpt-x"] {
            assert!(!openai(model), "{model}");
        }
    }

    #[test]
    fn tools_added_in_the_conversation_move_back_to_the_list_in_their_order() {
        let mut body = json!({
            "tools": [{"name": "Read"}],
            "input": [
                {"type": "message"},
                {"type": "additional_tools", "role": "developer", "tools": [{"name": "a"}]},
                {"type": "function_call_output"},
                {"type": "additional_tools", "role": "developer", "tools": [{"name": "b"}, {"name": "c"}]},
            ],
        });
        into_tools(&mut body);
        assert_eq!(body["tools"], json!([{"name": "Read"}, {"name": "a"}, {"name": "b"}, {"name": "c"}]));
        assert_eq!(body["input"], json!([{"type": "message"}, {"type": "function_call_output"}]));

        let mut plain = json!({"messages": [{"role": "user"}]});
        into_tools(&mut plain);
        assert_eq!(plain, json!({"messages": [{"role": "user"}]}), "other wires are left alone");
    }
}
