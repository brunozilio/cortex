use std::collections::HashMap;

use domain::egress::{Heard, Said};

use crate::transcript::{Block, Message, Role};

pub(in crate::agent) fn heard(history: &[Message]) -> Heard {
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
