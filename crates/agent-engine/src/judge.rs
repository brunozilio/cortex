use domain::Criteria;

use crate::transcript::{Message, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub criteria: Criteria,
    pub model: String,
}

pub const TOOLS: [&str; 5] = ["Read", "Glob", "Grep", "Bash", "Skill"];

pub const KIND: &str = "verification";

const MAX_STEPS: usize = 60;
const MAX_STEP_CHARS: usize = 300;
const MAX_REPLY_CHARS: usize = 8_000;

pub fn brief(criteria: &Criteria, run: &[Message]) -> String {
    let task = run.first().map(Message::text).unwrap_or_default();
    let steps: Vec<String> = run
        .iter()
        .filter(|message| message.role == Role::Assistant)
        .flat_map(|message| {
            message.tool_uses().map(|(_, name, input)| format!("- {name} {}", clip(&input.to_string(), MAX_STEP_CHARS)))
        })
        .collect();
    let skipped = steps.len().saturating_sub(MAX_STEPS);
    let steps = steps[skipped..].join("\n");
    let reply = run.iter().rev().find(|m| m.role == Role::Assistant).map(Message::text).unwrap_or_default();
    format!(
        "# Criteria\n{}\n\n# Task\n{}\n\n# The agent's tool calls{}\n{}\n\n# The agent's final message\n{}",
        criteria.text.trim(),
        task.trim(),
        if skipped > 0 { format!(" (the last {MAX_STEPS})") } else { String::new() },
        if steps.is_empty() { "(none)" } else { &steps },
        clip(reply.trim(), MAX_REPLY_CHARS),
    )
}

fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Block;

    #[test]
    fn the_brief_holds_the_criteria_the_task_the_steps_and_the_answer() {
        let run = vec![
            Message::user("write hello.txt"),
            Message::assistant(vec![Block::ToolUse {
                id: "1".into(),
                name: "Write".into(),
                input: serde_json::json!({"file_path":"hello.txt"}),
            }]),
            Message::assistant(vec![Block::Text("Wrote it".into())]),
        ];
        let brief = brief(&Criteria::user("hello.txt says oi"), &run);
        assert_eq!(
            brief,
            "# Criteria\nhello.txt says oi\n\n# Task\nwrite hello.txt\n\n# The agent's tool calls\n\
             - Write {\"file_path\":\"hello.txt\"}\n\n# The agent's final message\nWrote it"
        );
    }

    #[test]
    fn long_steps_are_clipped() {
        assert_eq!(clip("ábcdef", 3), "ábc…");
        assert_eq!(clip("abc", 3), "abc");
    }
}
