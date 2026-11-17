use serde_json::{Value, json};

pub const WEB_SEARCH: &str = "WebSearch";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hit {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub snippet: String,
}

const FILTERING: &[&str] = &["fable", "mythos", "opus-5", "opus-4-8", "opus-4-7", "opus-4-6", "sonnet-5", "sonnet-4-6"];

pub fn anthropic(model: &str) -> Value {
    let version = if FILTERING.iter().any(|family| model.contains(family)) {
        "web_search_20260209"
    } else {
        "web_search_20250305"
    };
    json!({ "type": version, "name": "web_search" })
}

pub fn responses() -> Value {
    json!({ "type": "web_search" })
}

pub fn put(native: Option<Value>, body: &mut Value) {
    let (Some(native), Some(tools)) = (native, body["tools"].as_array_mut()) else { return };
    let named = |tool: &Value| tool["name"] == WEB_SEARCH || tool["function"]["name"] == WEB_SEARCH;
    if let Some(tool) = tools.iter_mut().find(|tool| named(tool)) {
        let mut replaced = native;
        if let Some(cache) = tool.get("cache_control") {
            replaced["cache_control"] = cache.clone();
        }
        *tool = replaced;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_claude_models_get_the_filtering_search() {
        assert_eq!(anthropic("claude-opus-5-5")["type"], "web_search_20260209");
        assert_eq!(anthropic("claude-sonnet-4-6")["type"], "web_search_20260209");
        assert_eq!(anthropic("claude-haiku-4-5")["type"], "web_search_20250305");
    }

    #[test]
    fn the_function_tool_is_replaced_by_the_providers_own() {
        let mut body = json!({ "tools": [
            { "name": "Read", "input_schema": {} },
            { "name": WEB_SEARCH, "input_schema": {}, "cache_control": { "type": "ephemeral" } },
        ]});
        put(Some(anthropic("claude-opus-5-5")), &mut body);
        assert_eq!(body["tools"][0]["name"], "Read");
        assert_eq!(
            body["tools"][1],
            json!({ "type": "web_search_20260209", "name": "web_search", "cache_control": { "type": "ephemeral" } })
        );

        let mut responses_body = json!({ "tools": [{ "type": "function", "name": WEB_SEARCH, "parameters": {} }] });
        put(Some(responses()), &mut responses_body);
        assert_eq!(responses_body["tools"], json!([{ "type": "web_search" }]));
    }

    #[test]
    fn without_a_native_search_the_tool_is_left_as_it_is() {
        let mut body = json!({ "tools": [{ "name": WEB_SEARCH }] });
        put(None, &mut body);
        assert_eq!(body["tools"][0]["name"], WEB_SEARCH);
    }
}
