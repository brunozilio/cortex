use crate::transcript::{Block, Message, Role};

pub const INTERRUPTED: &str = "This tool call was interrupted before it ran.";

fn answers(block: &Block, call: &str) -> bool {
    matches!(block, Block::ToolResult { id, .. } | Block::ToolResultRich { id, .. } if id == call)
}

fn is_result(block: &Block) -> bool {
    matches!(block, Block::ToolResult { .. } | Block::ToolResultRich { .. })
}

fn unanswered(asked: &Message, answer: Option<&Message>) -> Vec<Block> {
    asked
        .tool_uses()
        .filter(|(call, _, _)| !answer.is_some_and(|answer| answer.blocks.iter().any(|block| answers(block, call))))
        .map(|(call, _, _)| Block::ToolResult { id: call.to_owned(), content: INTERRUPTED.into(), is_error: true })
        .collect()
}

pub fn repaired(history: Vec<Message>) -> Vec<Message> {
    let mut fixed: Vec<Message> = Vec::with_capacity(history.len() + 1);
    let mut messages = history.into_iter().peekable();
    while let Some(message) = messages.next() {
        let answer = messages.peek().filter(|next| next.role == Role::User);
        let missing = if message.role == Role::Assistant { unanswered(&message, answer) } else { Vec::new() };
        let answered = answer.is_some();
        fixed.push(message);
        if missing.is_empty() {
            continue;
        }
        match messages.next().filter(|_| answered) {
            Some(mut answer) => {
                let after = answer.blocks.iter().rposition(is_result).map_or(0, |last| last + 1);
                answer.blocks.splice(after..after, missing);
                fixed.push(answer);
            }
            None => fixed.push(Message { role: Role::User, blocks: missing }),
        }
    }
    fixed
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn call(ids: &[&str]) -> Message {
        Message::assistant(
            ids.iter().map(|id| Block::ToolUse { id: (*id).into(), name: "Read".into(), input: json!({}) }).collect(),
        )
    }

    fn result(id: &str, content: &str, is_error: bool) -> Block {
        Block::ToolResult { id: id.into(), content: content.into(), is_error }
    }

    #[test]
    fn a_call_left_without_a_result_is_answered_as_interrupted() {
        let history = vec![Message::user("go"), call(&["a"])];
        let fixed = repaired(history.clone());
        assert_eq!(fixed[..2], history[..]);
        assert_eq!(fixed[2], Message { role: Role::User, blocks: vec![result("a", INTERRUPTED, true)] });
    }

    #[test]
    fn only_the_missing_results_are_added_before_what_the_user_said_next() {
        let answered = Message { role: Role::User, blocks: vec![result("a", "ok", false), Block::Text("and?".into())] };
        let fixed = repaired(vec![Message::user("go"), call(&["a", "b"]), answered, call(&["c"]), Message::user("x")]);
        assert_eq!(
            fixed[2].blocks,
            vec![result("a", "ok", false), result("b", INTERRUPTED, true), Block::Text("and?".into())]
        );
        assert_eq!(fixed[4].blocks, vec![result("c", INTERRUPTED, true), Block::Text("x".into())]);
        assert_eq!(fixed.len(), 5);
    }

    #[test]
    fn a_complete_conversation_is_left_as_it_is() {
        let rich =
            Block::ToolResultRich { id: "b".into(), content: "seen".into(), images: Vec::new(), is_error: false };
        let history = vec![
            Message::user("go"),
            call(&["a", "b"]),
            Message { role: Role::User, blocks: vec![result("a", "ok", false), rich] },
            Message::assistant(vec![Block::Text("done".into())]),
        ];
        assert_eq!(repaired(history.clone()), history);
    }
}
