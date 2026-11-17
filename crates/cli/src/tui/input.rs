#[derive(Debug, Default)]
pub struct Input {
    text: String,
    cursor: usize,
    history: Vec<String>,
    recalled: Option<usize>,
    draft: (String, usize),
}

impl Input {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn set(&mut self, text: &str) {
        self.text = text.to_owned();
        self.cursor = self.text.len();
    }

    pub fn insert(&mut self, c: char) {
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.recalled = None;
    }

    pub fn insert_str(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.recalled = None;
    }

    pub fn backspace(&mut self) {
        if let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
            self.text.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
        }
    }

    pub fn right(&mut self) {
        if let Some(c) = self.text[self.cursor..].chars().next() {
            self.cursor += c.len_utf8();
        }
    }

    pub fn home(&mut self) {
        self.cursor = self.text[..self.cursor].rfind('\n').map_or(0, |at| at + 1);
    }

    pub fn end(&mut self) {
        self.cursor = self.text[self.cursor..].find('\n').map_or(self.text.len(), |at| self.cursor + at);
    }

    pub fn word_left(&mut self) {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end_matches(|c: char| !c.is_alphanumeric());
        self.cursor = trimmed.rfind(|c: char| !c.is_alphanumeric()).map_or(0, |at| at + 1);
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor += 1;
        }
    }

    pub fn word_right(&mut self) {
        let after = &self.text[self.cursor..];
        let skipped = after.len() - after.trim_start_matches(|c: char| !c.is_alphanumeric()).len();
        let rest = &after[skipped..];
        let word = rest.find(|c: char| !c.is_alphanumeric()).unwrap_or(rest.len());
        self.cursor += skipped + word;
    }

    pub fn delete_word_forward(&mut self) {
        let start = self.cursor;
        self.word_right();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub fn delete_to_line_start(&mut self) {
        let start = self.text[..self.cursor].rfind('\n').map_or(0, |at| at + 1);
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub fn on_first_line(&self) -> bool {
        !self.text[..self.cursor].contains('\n')
    }

    pub fn on_last_line(&self) -> bool {
        !self.text[self.cursor..].contains('\n')
    }

    pub fn line(&mut self, up: bool) {
        let start = self.text[..self.cursor].rfind('\n').map_or(0, |at| at + 1);
        let column = self.text[start..self.cursor].chars().count();
        let target = if up {
            let Some(end) = start.checked_sub(1) else { return };
            self.text[..end].rfind('\n').map_or(0, |at| at + 1)
        } else {
            let Some(next) = self.text[self.cursor..].find('\n') else { return };
            self.cursor + next + 1
        };
        let line_end = self.text[target..].find('\n').map_or(self.text.len(), |at| target + at);
        let line = &self.text[target..line_end];
        self.cursor = target + line.char_indices().nth(column).map_or(line.len(), |(at, _)| at);
    }

    pub fn set_history(&mut self, history: Vec<String>) {
        let earlier = history.len();
        let mut merged = history;
        merged.append(&mut self.history);
        self.history = merged;
        self.recalled = self.recalled.map(|at| at + earlier);
    }

    pub fn complete_word(&mut self, start: usize, word: &str) {
        let rest = self.text[self.cursor..].trim_start_matches(' ');
        let end = self.text.len() - rest.len();
        let completed = format!("{word} ");
        self.text.replace_range(start..end, &completed);
        self.cursor = start + completed.len();
    }

    pub fn delete_word(&mut self) {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end();
        let start = after_last_whitespace(trimmed);
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.recalled = None;
        if !text.trim().is_empty() && self.history.last() != Some(&text) {
            self.history.push(text.clone());
        }
        text
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.recalled = None;
    }

    pub fn older(&mut self) {
        let next = match self.recalled {
            None => self.history.len().checked_sub(1),
            Some(at) => at.checked_sub(1).or(Some(at)),
        };
        if let Some(at) = next {
            if self.recalled.is_none() {
                self.draft = (self.text.clone(), self.cursor);
            }
            self.recalled = Some(at);
            self.set(&self.history[at].clone());
        }
    }

    pub fn newer(&mut self) {
        match self.recalled {
            Some(at) if at + 1 < self.history.len() => {
                self.recalled = Some(at + 1);
                self.set(&self.history[at + 1].clone());
            }
            Some(_) => {
                self.recalled = None;
                (self.text, self.cursor) = std::mem::take(&mut self.draft);
            }
            None => {}
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn browsing_history(&self) -> bool {
        self.recalled.and_then(|at| self.history.get(at)).is_some_and(|recalled| *recalled == self.text)
    }

    pub fn slash_word(&self) -> Option<&str> {
        if self.browsing_history() {
            return None;
        }
        let word = self.text.strip_prefix('/')?;
        (!word.contains(char::is_whitespace)).then_some(word)
    }
}

pub fn after_last_whitespace(text: &str) -> usize {
    text.char_indices().rev().find(|(_, c)| c.is_whitespace()).map_or(0, |(at, c)| at + c.len_utf8())
}

#[cfg(test)]
#[path = "input_properties.rs"]
mod properties;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_moves_by_whole_characters() {
        let mut input = Input::default();
        for c in "olá mundo".chars() {
            input.insert(c);
        }
        input.left();
        input.left();
        input.backspace();
        assert_eq!(input.text(), "olá mudo");
        input.home();
        input.right();
        input.right();
        input.delete();
        assert_eq!(input.text(), "ol mudo");
        input.end();
        input.delete_word();
        assert_eq!(input.text(), "ol ");
    }

    #[test]
    fn deleting_a_word_after_a_multibyte_space_keeps_the_space() {
        for space in ['\u{a0}', '\u{3000}'] {
            let mut input = Input::default();
            input.set(&format!("olá{space}mundo"));
            input.delete_word();
            assert_eq!(input.text(), format!("olá{space}"));
            assert_eq!(input.cursor(), input.text().len());
        }
    }

    #[test]
    fn words_and_lines_are_jumped_over() {
        let mut input = Input::default();
        input.set("cargo test --all\nnext line");
        input.home();
        input.line(true);
        input.word_right();
        assert_eq!(input.cursor(), "cargo".len());
        input.word_right();
        input.word_right();
        assert_eq!(&input.text()[..input.cursor()], "cargo test --all");
        input.word_left();
        assert_eq!(&input.text()[..input.cursor()], "cargo test --");
        input.line(false);
        assert_eq!(&input.text()[input.cursor()..], "", "a shorter line below: its end");
        input.line(true);
        input.home();
        input.line(false);
        input.word_right();
        assert_eq!(&input.text()[input.cursor()..], " line", "same column on the next line");
        input.delete_to_line_start();
        assert_eq!(input.text(), "cargo test --all\n line");
        input.delete_word_forward();
        assert_eq!(input.text(), "cargo test --all\n");
        assert!(input.on_last_line() && !input.on_first_line());
    }

    #[test]
    fn earlier_chats_history_comes_first() {
        let mut input = Input::default();
        input.set("now");
        input.take();
        input.set_history(vec!["before".into()]);
        input.older();
        assert_eq!(input.text(), "now");
        input.older();
        assert_eq!(input.text(), "before");
    }

    #[test]
    fn history_recalls_sent_prompts_and_returns_to_a_new_one() {
        let mut input = Input::default();
        for prompt in ["first", "second"] {
            input.set(prompt);
            input.take();
        }
        input.older();
        assert_eq!(input.text(), "second");
        input.older();
        input.older();
        assert_eq!(input.text(), "first", "stops at the oldest");
        input.newer();
        assert_eq!(input.text(), "second");
        input.newer();
        assert_eq!(input.text(), "");
    }

    #[test]
    fn up_then_down_brings_back_the_half_typed_prompt() {
        let mut input = Input::default();
        input.set("sent");
        input.take();
        input.set("half typed");
        input.left();
        input.older();
        assert_eq!(input.text(), "sent");
        input.newer();
        assert_eq!((input.text(), input.cursor()), ("half typed", "half type".len()));
    }

    #[test]
    fn earlier_history_arriving_while_browsing_keeps_the_recalled_prompt() {
        let mut input = Input::default();
        input.set("now");
        input.take();
        input.older();
        input.set_history(vec!["before".into()]);
        assert!(input.browsing_history(), "still on the recalled prompt");
        input.newer();
        assert_eq!(input.text(), "", "down from the newest goes back to the draft");
    }

    #[test]
    fn a_slash_word_is_offered_for_completion_only_alone() {
        let mut input = Input::default();
        input.set("/mod");
        assert_eq!(input.slash_word(), Some("mod"));
        input.set("/model opus");
        assert_eq!(input.slash_word(), None);
        input.set("hello");
        assert_eq!(input.slash_word(), None);
    }
}
