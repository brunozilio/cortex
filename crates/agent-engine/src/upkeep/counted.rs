use super::message_tokens;
use crate::transcript::{Message, Usage};

#[derive(Debug, Default, Clone, Copy)]
pub struct Counted {
    last: Option<(u64, usize)>,
}

impl Counted {
    pub fn observe(&mut self, usage: Usage, history_len: usize) {
        self.last = (usage.prompt() > 0).then_some((usage.prompt() + usage.output, history_len));
    }

    pub fn forget(&mut self) {
        self.last = None;
    }

    pub fn used(&self, estimated: u64, history: &[Message]) -> u64 {
        match self.last {
            Some((tokens, at)) if at <= history.len() => tokens + history[at..].iter().map(message_tokens).sum::<u64>(),
            _ => estimated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(prompt: u64) -> Usage {
        Usage { input: prompt, output: 100, cache_read: 0, cache_write: 0 }
    }

    #[test]
    fn the_provider_s_count_plus_what_came_after_it_wins_over_the_estimate() {
        let mut counted = Counted::default();
        let history = vec![Message::user("a"), Message::assistant(vec![]), Message::user("x".repeat(400))];
        assert_eq!(counted.used(7, &history), 7, "nothing counted yet");
        counted.observe(usage(50_000), 2);
        assert_eq!(counted.used(7, &history), 50_100 + 100);
    }

    #[test]
    fn a_provider_that_counts_nothing_or_a_compacted_history_falls_back_to_the_estimate() {
        let mut counted = Counted::default();
        counted.observe(usage(0), 1);
        assert_eq!(counted.used(7, &[Message::user("a"), Message::user("b")]), 7, "no prompt count, no trust");
        counted.observe(usage(9), 5);
        assert_eq!(counted.used(7, &[Message::user("a")]), 7, "the history is shorter than when it was counted");
        counted.forget();
        assert_eq!(counted.used(7, &[]), 7);
    }
}
