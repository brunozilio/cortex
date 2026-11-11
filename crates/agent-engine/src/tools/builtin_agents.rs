use super::subagent::FoundAgent;
use super::subagent_config::AgentConfig;
use crate::transcript::{Block, Message, Role};

pub const FORK: &str = "fork";

const WRITERS: [&str; 4] = ["Agent", "Edit", "Write", "StructuredOutput"];

pub fn fork_preamble() -> String {
    content::text("agents/fork.md").split_once("\n---\n").map(|(_, body)| body.trim().to_owned()).unwrap_or_default()
}

pub fn general() -> String {
    content::text("prompts/agent-general.md")
}

pub fn is_fork(input: &serde_json::Value) -> bool {
    input["subagent_type"].as_str() == Some(FORK)
}

pub fn fork_history(parent: &[Message], directive: &str) -> Vec<Message> {
    let mut history = parent.to_vec();
    let mut blocks: Vec<Block> = parent
        .last()
        .filter(|message| message.role == Role::Assistant)
        .map(|message| {
            message
                .tool_uses()
                .map(|(id, _, _)| Block::ToolResult {
                    id: id.to_owned(),
                    content: "Fork started: it runs apart and reports back.".into(),
                    is_error: false,
                })
                .collect()
        })
        .unwrap_or_default();
    blocks.push(Block::Text(format!("{}\n{directive}", fork_preamble())));
    history.push(Message { role: Role::User, blocks });
    history
}

fn read_only() -> AgentConfig {
    AgentConfig { disallowed_tools: WRITERS.map(str::to_owned).to_vec(), omit_instructions: true, ..Default::default() }
}

pub fn all() -> Vec<FoundAgent> {
    content::builtin_files("agents/")
        .into_iter()
        .filter_map(|(path, _)| {
            let text = content::text(path);
            let (front, body) = crate::frontmatter::parse(&text);
            let read_only = front.get("read-only") == Some("true");
            Some(FoundAgent {
                name: front.get("name")?.to_owned(),
                description: front.get("description").unwrap_or_default().to_owned(),
                instructions: body.trim().to_owned(),
                config: if read_only { self::read_only() } else { AgentConfig::default() },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_agents_cannot_write_and_none_names_another_product() {
        for agent in all() {
            assert!(!agent.instructions.contains("Claude"), "{}", agent.name);
            assert!(!agent.instructions.contains("{READ_ONLY}"), "{}", agent.name);
        }
        let named = |name: &str| all().into_iter().find(|agent| agent.name == name).expect("built in");
        let explore = &named("Explore");
        assert!(explore.config.allows("Read") && explore.config.allows("Bash"));
        assert!(!explore.config.allows("Edit") && !explore.config.allows("Write") && !explore.config.allows("Agent"));
        assert!(explore.config.omit_instructions);
        assert!(named(FORK).config.allows("Edit"), "a fork keeps the parent's tools");
    }

    #[test]
    fn a_fork_answers_the_parents_calls_and_then_reads_its_directive() {
        let call = |id: &str| Block::ToolUse {
            id: id.into(),
            name: "Agent".into(),
            input: serde_json::json!({"subagent_type": "fork", "prompt": "x"}),
        };
        let parent = vec![Message::user("Ship it"), Message::assistant(vec![call("a"), call("b")])];
        let history = fork_history(&parent, "Check the tests");
        assert_eq!(history.len(), 3);
        assert_eq!(&history[..2], &parent[..]);
        let answers: Vec<&str> = history[2]
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::ToolResult { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(answers, ["a", "b"]);
        assert!(history[2].text().ends_with("Your directive:\nCheck the tests"));
        assert!(is_fork(&serde_json::json!({"subagent_type": "fork"})));
        assert!(!is_fork(&serde_json::json!({"subagent_type": "Explore"})));
    }
}
