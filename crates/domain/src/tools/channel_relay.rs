use serde_json::{Value, json};

use crate::tool_name::MCP_PREFIX;

pub const PERMISSION_CAPABILITY: &str = "claude/channel/permission";
pub const PERMISSION_REQUEST: &str = "notifications/claude/channel/permission_request";
pub const PERMISSION_ANSWER: &str = "notifications/claude/channel/permission";
pub const PREVIEW: usize = 200;

const LETTERS: &[u8] = b"abcdefghijkmnopqrstuvwxyz";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relayed {
    pub request_id: String,
    pub allow: bool,
}

pub fn request_id(call: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in call.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let base = LETTERS.len() as u64;
    (0..5)
        .map(|_| {
            let letter = LETTERS[usize::try_from(hash % base).unwrap_or_default()];
            hash /= base;
            char::from(letter)
        })
        .collect()
}

pub fn permission_request(request_id: &str, tool: &str, description: &str, input: &Value) -> Value {
    let input = input.to_string();
    let input_preview = match input.char_indices().nth(PREVIEW) {
        Some((at, _)) => format!("{}…", &input[..at]),
        None => input,
    };
    json!({
        "jsonrpc": "2.0",
        "method": PERMISSION_REQUEST,
        "params": {
            "request_id": request_id,
            "tool_name": tool,
            "description": description,
            "input_preview": input_preview,
        }
    })
}

pub fn permission_answer(method: &str, params: &Value) -> Option<Relayed> {
    if method != PERMISSION_ANSWER {
        return None;
    }
    let request_id = params["request_id"].as_str()?.trim().to_ascii_lowercase();
    if request_id.is_empty() {
        return None;
    }
    let allow = match params["behavior"].as_str()? {
        "allow" => true,
        "deny" => false,
        _ => return None,
    };
    Some(Relayed { request_id, allow })
}

pub fn asks_for_its_own_tool(server: &str, tool: &str) -> bool {
    tool.strip_prefix(MCP_PREFIX).and_then(|rest| rest.strip_prefix(server)).is_some_and(|rest| rest.starts_with("__"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_request_id_is_five_letters_a_person_can_type_and_the_same_for_the_same_call() {
        let id = request_id("toolu_01ABC");
        assert_eq!(id.len(), 5);
        assert!(id.chars().all(|c| c.is_ascii_lowercase() && c != 'l'), "{id}");
        assert_eq!(id, request_id("toolu_01ABC"));
        assert_ne!(id, request_id("toolu_01ABD"));
    }

    #[test]
    fn a_permission_request_names_the_tool_what_it_will_do_and_a_preview_of_its_input() {
        let input = json!({ "command": "x".repeat(400) });
        let sent = permission_request("abcde", "Bash", "Run a long command", &input);
        assert_eq!(sent["jsonrpc"], "2.0");
        assert_eq!(sent["method"], PERMISSION_REQUEST);
        let params = &sent["params"];
        assert_eq!(params["request_id"], "abcde");
        assert_eq!(params["tool_name"], "Bash");
        assert_eq!(params["description"], "Run a long command");
        let preview = params["input_preview"].as_str().expect("preview");
        assert!(preview.starts_with("{\"command\":\"xxx") && preview.chars().count() <= PREVIEW + 1, "{preview}");
    }

    #[test]
    fn an_answer_is_read_only_from_the_permission_method_with_allow_or_deny() {
        let allow = json!({ "request_id": " ABCDE ", "behavior": "allow" });
        assert_eq!(
            permission_answer(PERMISSION_ANSWER, &allow),
            Some(Relayed { request_id: "abcde".into(), allow: true })
        );
        let deny = json!({ "request_id": "abcde", "behavior": "deny" });
        assert_eq!(
            permission_answer(PERMISSION_ANSWER, &deny),
            Some(Relayed { request_id: "abcde".into(), allow: false })
        );
        assert_eq!(permission_answer(PERMISSION_ANSWER, &json!({ "request_id": "abcde", "behavior": "maybe" })), None);
        assert_eq!(permission_answer(PERMISSION_ANSWER, &json!({ "behavior": "allow" })), None);
        assert_eq!(permission_answer(crate::inbox::CHANNEL_METHOD, &allow), None);
    }

    #[test]
    fn a_channel_cannot_allow_its_own_tools() {
        assert!(asks_for_its_own_tool("tg", "mcp__tg__reply"));
        assert!(!asks_for_its_own_tool("tg", "mcp__tgx__reply"));
        assert!(!asks_for_its_own_tool("tg", "Bash"));
    }
}
