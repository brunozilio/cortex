use std::collections::HashMap;

use super::tokens;
use crate::transcript::{Block, Message};

const CLEARABLE: [&str; 8] = ["Read", "Grep", "Glob", "Bash", "BashOutput", "WebFetch", "WebSearch", "LSP"];
const KEPT_RECENT: usize = 6;
const WORTH_CLEARING: usize = 500;
pub const CLEARED: &str = "[Old tool result cleared to save context; run the tool again if you need it.]";

pub fn clear_old_results(history: &mut [Message]) -> u64 {
    let names: HashMap<String, String> = history
        .iter()
        .flat_map(|message| message.tool_uses().map(|(id, name, _)| (id.to_owned(), name.to_owned())))
        .collect();
    let mut results: Vec<&mut String> = history
        .iter_mut()
        .flat_map(|message| message.blocks.iter_mut())
        .filter_map(|block| match block {
            Block::ToolResult { id, content, .. } | Block::ToolResultRich { id, content, .. } => {
                let clearable = names.get(id.as_str()).is_some_and(|name| CLEARABLE.contains(&name.as_str()));
                clearable.then_some(content)
            }
            _ => None,
        })
        .collect();
    let old = results.len().saturating_sub(KEPT_RECENT);
    results
        .iter_mut()
        .take(old)
        .filter(|content| content.len() >= WORTH_CLEARING)
        .map(|content| {
            let freed = tokens(content).saturating_sub(tokens(CLEARED));
            **content = CLEARED.to_owned();
            freed
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn turn(id: &str, tool: &str, size: usize) -> [Message; 2] {
        let call = Block::ToolUse { id: id.into(), name: tool.into(), input: json!({}) };
        let result = Block::ToolResult { id: id.into(), content: "r".repeat(size), is_error: false };
        [Message::assistant(vec![call]), Message { role: crate::transcript::Role::User, blocks: vec![result] }]
    }

    fn contents(history: &[Message]) -> Vec<String> {
        history
            .iter()
            .flat_map(|m| &m.blocks)
            .filter_map(|b| if let Block::ToolResult { content, .. } = b { Some(content.clone()) } else { None })
            .collect()
    }

    #[test]
    fn old_large_reads_are_cleared_and_the_latest_ones_kept() {
        let mut history: Vec<Message> = (0..8).flat_map(|i| turn(&format!("t{i}"), "Read", 4_000)).collect();
        let freed = clear_old_results(&mut history);
        let contents = contents(&history);
        assert_eq!(contents.iter().filter(|c| *c == CLEARED).count(), 2);
        assert_eq!(contents[0], CLEARED);
        assert_eq!(contents[7].len(), 4_000);
        assert!(freed > 1_900, "{freed}");
    }

    #[test]
    fn edits_agents_and_small_results_stay() {
        let mut history: Vec<Message> = turn("e", "Edit", 4_000)
            .into_iter()
            .chain(turn("a", "Agent", 4_000))
            .chain(turn("s", "Read", 100))
            .chain((0..6).flat_map(|i| turn(&format!("t{i}"), "Read", 4_000)))
            .collect();
        assert_eq!(clear_old_results(&mut history), 0);
        assert!(!contents(&history).contains(&CLEARED.to_owned()));
    }
}
