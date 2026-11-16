use std::collections::HashMap;

use domain::egress::{Heard, Said};
use serde_json::Value;

use super::Agent;
use crate::transcript::{Block, Message, Role};

const FETCHING_TOOL: &str = "WebFetch";

pub(super) fn heard(history: &[Message]) -> Heard {
    let tools: HashMap<&str, &str> = history
        .iter()
        .flat_map(|message| &message.blocks)
        .filter_map(|block| match block {
            Block::ToolUse { id, name, .. } => Some((id.as_str(), name.as_str())),
            _ => None,
        })
        .collect();
    let said: Vec<Said<'_>> = history
        .iter()
        .filter(|message| message.role == Role::User)
        .flat_map(|message| &message.blocks)
        .filter_map(|block| match block {
            Block::Text(text) => Some(Said::User(text)),
            Block::ToolResult { id, content, .. } => {
                Some(Said::Result { tool: tools.get(id.as_str()).copied().unwrap_or_default(), text: content })
            }
            _ => None,
        })
        .collect();
    Heard::of(&said)
}

impl Agent<'_> {
    pub(super) fn always_asks(&self, tool: &str, input: &Value) -> Option<String> {
        if let domain::command_policy::CommandClass::Risky(why) = self.command_class(tool, input) {
            return Some(why);
        }
        let url = input["url"].as_str().filter(|_| tool == FETCHING_TOOL)?;
        self.workspace.tracking.heard().concern(url).map(|why| format!("Check this address first: {why}."))
    }
}
