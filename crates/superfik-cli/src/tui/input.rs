//! The prompt being typed: text with a cursor, and the prompts sent
//! before (Up and Down recall them).

#[derive(Debug, Default)]
pub struct Input {
    text: String,
    /// Byte offset, always on a character boundary.
    cursor: usize,
    history: Vec<String>,
    /// Which sent prompt Up reached; `None` while editing a new one.
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

    /// Deletes back to the start of the previous word (Ctrl+W).
    pub fn delete_word(&mut self) {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end();
        let start = trimmed.rfind(char::is_whitespace).map_or(0, |at| at + 1);
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    /// Takes the text to send and remembers it.
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

    /// Whether Up and Down move through history rather than lines.
    pub fn on_one_line(&self) -> bool {
        !self.text.contains('\n')
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

    /// The `/word` being typed when the whole input is one, for completion.
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
