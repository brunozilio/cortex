use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use crate::added_tools::{TOOL_SEARCH, loaded_names};
use crate::model::{ToolSpec, TurnRequest};
use crate::transcript::Block;

#[derive(Debug, Default)]
pub struct Loaded {
    after: HashMap<String, Vec<Value>>,
    names: HashSet<String>,
}

impl Loaded {
    pub fn of(request: &TurnRequest) -> Self {
        let searches: HashSet<&str> = request
            .messages
            .iter()
            .flat_map(|message| &message.blocks)
            .filter_map(|block| match block {
                Block::ToolUse { id, name, .. } if name == TOOL_SEARCH => Some(id.as_str()),
                _ => None,
            })
            .collect();
        let mut loaded = Self::default();
        for block in request.messages.iter().flat_map(|message| &message.blocks) {
            let Block::ToolResult { id, content, is_error: false } = block else { continue };
            if !searches.contains(id.as_str()) {
                continue;
            }
            let tools: Vec<Value> = loaded_names(content)
                .into_iter()
                .filter_map(|name| request.tools.iter().find(|spec| spec.name == name))
                .filter(|spec| loaded.names.insert(spec.name.clone()))
                .map(tool)
                .collect();
            if !tools.is_empty() {
                loaded.after.insert(id.clone(), tools);
            }
        }
        loaded
    }

    pub fn declared(&self, spec: &ToolSpec) -> bool {
        !self.names.contains(&spec.name)
    }

    pub fn after(&self, call_id: &str) -> Option<Value> {
        let tools = self.after.get(call_id)?;
        Some(json!({ "type": "additional_tools", "role": "developer", "tools": tools }))
    }
}

pub fn tool(spec: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "name": spec.name,
        "description": spec.description,
        "parameters": spec.schema,
        "strict": false,
    })
}

#[cfg(test)]
mod tests {
    use domain::Effort;
    use serde_json::json;

    use crate::model::wires::responses::body;
    use crate::model::{ToolSpec, TurnRequest};
    use crate::transcript::{Block, Message, Role};

    fn spec(name: &str) -> ToolSpec {
        ToolSpec { name: name.into(), description: format!("{name} does it"), schema: json!({"type": "object"}) }
    }

    fn found(names: &[&str]) -> String {
        let shown: Vec<_> =
            names.iter().map(|name| json!({"name": name, "description": "", "parameters": {}})).collect();
        format!("Loaded; call them from now on:\n{}", serde_json::to_string_pretty(&shown).unwrap_or_default())
    }

    fn results(blocks: Vec<Block>) -> Message {
        Message { role: Role::User, blocks }
    }

    #[test]
    fn a_tool_a_search_loaded_joins_the_conversation_where_it_was_loaded_and_leaves_the_tools_list() {
        let user_info = "mcp__jira__atlassianUserInfo";
        let request = TurnRequest {
            model: "gpt-5.6-sol".into(),
            system: "Be brief.".into(),
            messages: vec![
                Message::user("who am I"),
                Message::assistant(vec![
                    Block::ToolUse { id: "s1".into(), name: "ToolSearch".into(), input: json!({"query": "x"}) },
                    Block::ToolUse { id: "s2".into(), name: "ToolSearch".into(), input: json!({"query": "y"}) },
                    Block::ToolUse { id: "r1".into(), name: "Read".into(), input: json!({}) },
                ]),
                results(vec![
                    Block::ToolResult {
                        id: "s1".into(),
                        content: found(&[user_info, "mcp__other__gone"]),
                        is_error: false,
                    },
                    Block::ToolResult { id: "s2".into(), content: found(&[user_info]), is_error: false },
                    Block::ToolResult { id: "r1".into(), content: found(&["Read"]), is_error: false },
                ]),
            ],
            tools: vec![spec("Read"), spec("ToolSearch"), spec(user_info)],
            effort: Effort::Medium,
            max_tokens: 1000,
        };
        let body = body(&request);
        let declared: Vec<&str> =
            body["tools"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(declared, ["Read", "ToolSearch"], "the list a cache starts with stays the same");
        let input = body["input"].as_array().expect("input");
        let at = input.iter().position(|item| item["call_id"] == "s1" && item["type"] == "function_call_output");
        let added = &input[at.expect("s1's result") + 1];
        assert_eq!(added["type"], "additional_tools");
        assert_eq!(added["role"], "developer");
        assert_eq!(
            added["tools"],
            json!([{"type": "function", "name": user_info, "description": format!("{user_info} does it"), "parameters": {"type": "object"}, "strict": false}])
        );
        let added_count = input.iter().filter(|item| item["type"] == "additional_tools").count();
        assert_eq!(added_count, 1, "a tool joins once, at its first load; Read was never a search");
    }

    #[test]
    fn a_tool_whose_search_is_no_longer_in_the_conversation_stays_declared() {
        let request = TurnRequest {
            model: "gpt-5.6-sol".into(),
            system: String::new(),
            messages: vec![
                Message::user("go"),
                Message::assistant(vec![Block::ToolUse {
                    id: "s1".into(),
                    name: "ToolSearch".into(),
                    input: json!({}),
                }]),
                results(vec![Block::ToolResult { id: "s1".into(), content: found(&["mcp__a__b"]), is_error: true }]),
            ],
            tools: vec![spec("ToolSearch"), spec("mcp__a__b")],
            effort: Effort::Medium,
            max_tokens: 1000,
        };
        let body = body(&request);
        assert_eq!(body["tools"].as_array().map(Vec::len), Some(2));
        assert!(!body["input"].to_string().contains("additional_tools"));
    }
}
