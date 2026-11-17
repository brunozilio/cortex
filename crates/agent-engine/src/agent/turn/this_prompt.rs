use crate::transcript::{Block, Message, Role};

#[derive(Debug)]
pub(super) struct ThisPrompt {
    asked: Vec<Block>,
    at: usize,
}

fn said(message: &Message) -> Vec<Block> {
    message.blocks.iter().filter(|block| !matches!(block, Block::Reminder(_))).cloned().collect()
}

impl ThisPrompt {
    pub(super) fn last_of(history: &[Message]) -> Self {
        Self { asked: history.last().map(said).unwrap_or_default(), at: history.len().saturating_sub(1) }
    }

    pub(super) fn compacted(&mut self, history: &[Message]) {
        self.at =
            history.iter().rposition(|message| message.role == Role::User && said(message) == self.asked).unwrap_or(0);
    }

    pub(super) fn run<'a>(&self, history: &'a [Message]) -> &'a [Message] {
        history.get(self.at..).unwrap_or(history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(text: &str) -> Message {
        Message::assistant(vec![Block::Text(text.into())])
    }

    #[test]
    fn the_run_starts_at_the_prompt_until_compaction_moves_it() {
        let mut history = vec![Message::user("one"), answer("1"), Message::user("two")];
        let mut prompt = ThisPrompt::last_of(&history);
        history.push(answer("2"));
        assert_eq!(prompt.run(&history), &history[2..]);

        let mut asked = Message::user("two");
        asked.blocks.push(Block::Reminder("today".into()));
        let compacted = vec![Message::user("summary"), asked, answer("2")];
        prompt.compacted(&compacted);
        assert_eq!(prompt.run(&compacted), &compacted[1..], "the prompt is found again, reminders aside");
    }

    #[test]
    fn a_prompt_the_summary_swallowed_leaves_the_whole_compacted_conversation() {
        let history: Vec<Message> = (0..9).map(|i| Message::user(format!("q{i}"))).collect();
        let mut prompt = ThisPrompt::last_of(&history);
        let compacted = vec![Message::user("summary"), answer("done")];
        assert_eq!(prompt.run(&compacted), &compacted[..], "an index past the end never slices");
        prompt.compacted(&compacted);
        assert_eq!(prompt.run(&compacted), &compacted[..]);
    }
}
