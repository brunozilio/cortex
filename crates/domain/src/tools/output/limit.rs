use serde::{Deserialize, Serialize};

pub const DEFAULT_LIMIT: usize = 30_000;
pub const CUT_NOTE: &str = "[Output cut: ";
pub const SMALLEST_LIMIT: usize = 1_000;
pub const LARGEST_LIMIT: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputLimit(usize);

impl Default for OutputLimit {
    fn default() -> Self {
        Self(DEFAULT_LIMIT)
    }
}

impl OutputLimit {
    pub fn of(characters: u64) -> Self {
        let wanted = usize::try_from(characters).unwrap_or(LARGEST_LIMIT);
        Self(wanted.clamp(SMALLEST_LIMIT, LARGEST_LIMIT))
    }

    pub fn characters(self) -> usize {
        self.0
    }

    pub fn exceeded_by(self, text: &str) -> bool {
        text.chars().nth(self.0).is_some()
    }

    pub fn cut(self, text: &str, saved_at: Option<&str>) -> String {
        if !self.exceeded_by(text) {
            return text.to_owned();
        }
        let total = text.chars().count();
        let lines = text.lines().count();
        let (head, tail) = (self.0 / 2, self.0 - self.0 / 2);
        let head_end = text.char_indices().nth(head).map_or(text.len(), |(at, _)| at);
        let tail_start = text.char_indices().nth(total - tail).map_or(text.len(), |(at, _)| at);
        let whole = match saved_at {
            Some(path) => format!("The full output is in {path}: Read or Grep it for the rest."),
            None => "The full output could not be saved.".to_owned(),
        };
        format!(
            "{CUT_NOTE}{total} characters in {lines} lines, over the limit of {}; the first {head} and the last \
             {tail} are shown. {whole}]\n{}\n… [{} characters cut here] …\n{}",
            self.0,
            &text[..head_end],
            total - self.0,
            &text[tail_start..],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_limit_is_kept_within_its_bounds() {
        assert_eq!(OutputLimit::default().characters(), DEFAULT_LIMIT);
        assert_eq!(OutputLimit::of(5).characters(), SMALLEST_LIMIT);
        assert_eq!(OutputLimit::of(u64::MAX).characters(), LARGEST_LIMIT);
        assert_eq!(OutputLimit::of(4_000).characters(), 4_000);
        assert_eq!(serde_json::to_value(OutputLimit::of(4_000)).expect("json"), serde_json::json!(4_000));
    }

    #[test]
    fn output_within_the_limit_is_left_alone() {
        let limit = OutputLimit::of(1_000);
        let text = "é".repeat(1_000);
        assert!(!limit.exceeded_by(&text));
        assert_eq!(limit.cut(&text, Some("/tmp/x.txt")), text);
    }

    #[test]
    fn a_cut_output_says_first_how_much_was_cut_and_where_the_whole_is_and_keeps_both_ends() {
        let limit = OutputLimit::of(1_000);
        let text = format!("START\n{}\nEND", "é\n".repeat(1_000));
        let cut = limit.cut(&text, Some("/tmp/results/bash-1.txt"));
        let (note, rest) = cut.split_once('\n').expect("note");
        assert_eq!(
            note,
            "[Output cut: 2010 characters in 1003 lines, over the limit of 1000; the first 500 and the last 500 are \
             shown. The full output is in /tmp/results/bash-1.txt: Read or Grep it for the rest.]"
        );
        assert!(rest.starts_with("START\n") && rest.ends_with("\nEND"), "{rest}");
        assert!(rest.contains("\n… [1010 characters cut here] …\n"));
        assert_eq!(rest.chars().count() - "\n… [1010 characters cut here] …\n".chars().count(), 1_000);
        assert!(limit.cut(&text, None).starts_with("[Output cut: 2010 characters"));
        assert!(limit.cut(&text, None).contains("The full output could not be saved.]"));
    }
}
