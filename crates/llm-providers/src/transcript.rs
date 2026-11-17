use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReasoningOrigin(pub std::borrow::Cow<'static, str>);

impl ReasoningOrigin {
    pub const fn of(wire: &'static str) -> Self {
        Self(std::borrow::Cow::Borrowed(wire))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Block {
    Text(String),
    Image(domain::Image),
    ToolResultRich {
        id: String,
        content: String,
        images: Vec<domain::Image>,
        is_error: bool,
    },
    Thinking {
        text: String,
        signature: Option<String>,
        #[serde(default)]
        origin: Option<ReasoningOrigin>,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
    Reminder(String),
    Native {
        origin: ReasoningOrigin,
        item: Value,
    },
}

impl Block {
    pub fn sent_text(&self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text.clone()),
            Self::Reminder(text) => Some(format!("<system-reminder>\n{}\n</system-reminder>", text.trim())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, blocks: vec![Block::Text(text.into())] }
    }

    pub fn assistant(blocks: Vec<Block>) -> Self {
        Self { role: Role::Assistant, blocks }
    }

    pub fn tool_uses(&self) -> impl Iterator<Item = (&str, &str, &Value)> {
        self.blocks.iter().filter_map(|block| match block {
            Block::ToolUse { id, name, input } => Some((id.as_str(), name.as_str(), input)),
            _ => None,
        })
    }

    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
}

pub fn replied(history: &mut Vec<Message>, reply: Message) -> bool {
    let said = reply.blocks.iter().any(|block| !matches!(block, Block::Text(text) if text.trim().is_empty()));
    if said {
        history.push(reply);
    }
    said
}

pub fn told(history: &mut Vec<Message>, user: Message) {
    match history.last_mut() {
        Some(last) if last.role == Role::User => last.blocks.extend(user.blocks),
        _ => history.push(user),
    }
}

pub fn only_results(message: &Message) -> bool {
    let is_result = |block: &Block| matches!(block, Block::ToolResult { .. } | Block::ToolResultRich { .. });
    message.role == Role::User && !message.blocks.is_empty() && message.blocks.iter().all(is_result)
}

pub fn prompted(history: &mut Vec<Message>, prompt: Message) {
    match history.last_mut() {
        Some(last) if only_results(last) => last.blocks.extend(prompt.blocks),
        _ => history.push(prompt),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal,
    PauseTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub searches: u64,
}

impl Usage {
    pub const fn prompt(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }

    pub fn add(&mut self, other: Usage) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.searches += other.searches;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_tool_calls_and_joins_text() {
        let message = Message::assistant(vec![
            Block::Text("Looking ".into()),
            Block::ToolUse { id: "t1".into(), name: "Read".into(), input: serde_json::json!({"file_path": "a"}) },
            Block::Text("now".into()),
        ]);
        assert_eq!(message.text(), "Looking now");
        let calls: Vec<_> = message.tool_uses().map(|(id, name, _)| (id, name)).collect();
        assert_eq!(calls, vec![("t1", "Read")]);
    }

    #[test]
    fn a_reply_with_nothing_in_it_is_not_kept_as_an_assistant_message() {
        let mut history = vec![Message::user("go")];
        assert!(!replied(&mut history, Message::assistant(Vec::new())));
        assert!(!replied(&mut history, Message::assistant(vec![Block::Text(" \n".into())])));
        assert_eq!(history, [Message::user("go")]);
        let thought = Block::Thinking { text: "hm".into(), signature: None, origin: None };
        assert!(replied(&mut history, Message::assistant(vec![thought])));
        assert!(replied(&mut history, Message::assistant(vec![Block::Text("Done.".into())])));
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn a_prompt_after_results_left_unanswered_joins_them_so_roles_alternate() {
        let interrupted = Block::ToolResult { id: "t".into(), content: "cut".into(), is_error: true };
        let mut history = vec![
            Message::user("go"),
            Message::assistant(vec![Block::Text("On it.".into())]),
            Message { role: Role::User, blocks: vec![interrupted.clone()] },
        ];
        prompted(&mut history, Message::user("and then?"));
        assert_eq!(history.len(), 3);
        assert_eq!(history[2].blocks, [interrupted, Block::Text("and then?".into())]);

        history.push(Message::assistant(vec![Block::Text("Next.".into())]));
        prompted(&mut history, Message::user("thanks"));
        assert_eq!(history.len(), 5);
        assert_eq!(history[4], Message::user("thanks"));

        let mut unanswered = vec![Message::user("summary of what came before")];
        prompted(&mut unanswered, Message::user("go on"));
        assert_eq!(unanswered.len(), 2, "a prompt never joins what the user or a summary said before");
    }

    #[test]
    fn what_the_harness_tells_the_model_mid_run_joins_the_user_message_before_it() {
        let mut history = vec![Message::user("go")];
        told(&mut history, Message::user("be brief"));
        assert_eq!(
            history,
            [Message { role: Role::User, blocks: vec![Block::Text("go".into()), Block::Text("be brief".into())] }]
        );
        history.push(Message::assistant(vec![Block::Text("Ok.".into())]));
        told(&mut history, Message::user("more"));
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn usage_adds_up() {
        let mut total = Usage { input: 1, output: 2, cache_read: 3, cache_write: 4, searches: 0 };
        total.add(Usage { input: 1, output: 1, cache_read: 1, cache_write: 1, searches: 0 });
        assert_eq!(total.prompt(), 2 + 4 + 5);
        assert_eq!(total.output, 3);
    }
}
