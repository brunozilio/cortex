#[derive(Debug, Default)]
pub struct Input {
    text: String,
    cursor: usize,
    history: Vec<String>,
    recalled: Option<usize>,
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
        let mut merged = history;
        merged.append(&mut self.history);
        self.history = merged;
    }

    pub fn delete_word(&mut self) {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end();
        let start = trimmed.rfind(char::is_whitespace).map_or(0, |at| at + 1);
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
                self.set("");
            }
            None => {}
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn slash_word(&self) -> Option<&str> {
        let word = self.text.strip_prefix('/')?;
        (!word.contains(char::is_whitespace)).then_some(word)
    }
}

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
