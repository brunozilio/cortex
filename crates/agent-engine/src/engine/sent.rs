use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use serde_json::Value;

use crate::transcript::Message;

#[derive(Debug, Default)]
pub(super) struct Conversations {
    sent: HashMap<Option<usize>, Vec<u64>>,
}

#[derive(Debug, PartialEq)]
pub(super) struct Delta {
    pub keep: usize,
    pub added: Vec<Value>,
}

impl Conversations {
    pub(super) fn delta(&mut self, agent: Option<usize>, messages: &[Message]) -> Delta {
        let values: Vec<Value> =
            messages.iter().map(|message| serde_json::to_value(message).unwrap_or_default()).collect();
        let prints: Vec<u64> = values.iter().map(print).collect();
        let before = self.sent.entry(agent).or_default();
        let keep = before.iter().zip(&prints).take_while(|(old, new)| old == new).count();
        *before = prints;
        Delta { keep, added: values.into_iter().skip(keep).collect() }
    }
}

fn print(value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.to_string().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::transcript::Block;

    fn texts(delta: &Delta) -> Vec<String> {
        delta.added.iter().map(|message| message["blocks"][0]["Text"].as_str().unwrap_or_default().to_owned()).collect()
    }

    #[test]
    fn the_first_call_sends_everything_and_each_next_one_only_what_was_added() {
        let mut conversations = Conversations::default();
        let one = [Message::user("fix it")];
        let first = conversations.delta(None, &one);
        assert_eq!((first.keep, texts(&first)), (0, vec!["fix it".to_owned()]));

        let two = [Message::user("fix it"), Message::assistant(vec![Block::Text("on it".into())]), Message::user("go")];
        let second = conversations.delta(None, &two);
        assert_eq!((second.keep, texts(&second)), (1, vec!["on it".to_owned(), "go".to_owned()]));
    }

    #[test]
    fn a_rewritten_history_keeps_only_what_still_matches() {
        let mut conversations = Conversations::default();
        conversations.delta(None, &[Message::user("a"), Message::user("b"), Message::user("c")]);
        let compacted = conversations.delta(None, &[Message::user("a"), Message::user("summary")]);
        assert_eq!((compacted.keep, texts(&compacted)), (1, vec!["summary".to_owned()]));
    }

    #[test]
    fn each_agent_has_its_own_conversation() {
        let mut conversations = Conversations::default();
        conversations.delta(None, &[Message::user("main")]);
        let agent = conversations.delta(Some(0), &[Message::user("task")]);
        assert_eq!(agent.keep, 0);
        let main = conversations.delta(None, &[Message::user("main"), Message::user("more")]);
        assert_eq!((main.keep, main.added), (1, vec![json!({ "role": "User", "blocks": [{ "Text": "more" }] })]));
    }
}
