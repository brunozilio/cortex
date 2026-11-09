//! Editing model for single-field text input: caret, selection, edits.
//! Pure state; the composer view renders it and the app feeds it keys.

mod history;

use history::{History, Kind};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextEdit {
    text: String,
    caret: usize,
    anchor: Option<usize>,
    history: History,
}

impl TextEdit {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.len();
        Self { text, caret, anchor: None, history: History::default() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        let (a, b) = (anchor.min(self.caret), anchor.max(self.caret));
        (a != b).then_some((a, b))
    }

    pub fn insert(&mut self, s: &str) {
        self.checkpoint(Kind::Typing);
        self.delete_selection();
        self.text.insert_str(self.caret, s);
        self.caret += s.len();
    }

    pub fn backspace(&mut self) {
        self.checkpoint(Kind::Deleting);
        if self.delete_selection() {
            return;
        }
        if let Some(prev) = self.prev_boundary(self.caret) {
            self.text.replace_range(prev..self.caret, "");
            self.caret = prev;
        }
    }

    pub fn backspace_word(&mut self) {
        self.checkpoint(Kind::Other);
        if self.delete_selection() {
            return;
        }
        let start = self.word_start(self.caret);
        self.text.replace_range(start..self.caret, "");
        self.caret = start;
    }

    pub fn delete(&mut self) {
        self.checkpoint(Kind::Deleting);
        if self.delete_selection() {
            return;
        }
        if let Some(next) = self.next_boundary(self.caret) {
            self.text.replace_range(self.caret..next, "");
        }
    }

    pub fn move_left(&mut self, select: bool) {
        self.update_anchor(select);
        if let Some(prev) = self.prev_boundary(self.caret) {
            self.caret = prev;
        }
    }

    pub fn move_right(&mut self, select: bool) {
        self.update_anchor(select);
        if let Some(next) = self.next_boundary(self.caret) {
            self.caret = next;
        }
    }

    pub fn move_home(&mut self, select: bool) {
        self.update_anchor(select);
        self.caret = 0;
    }

    pub fn move_end(&mut self, select: bool) {
        self.update_anchor(select);
        self.caret = self.text.len();
    }

    pub fn move_to(&mut self, offset: usize, select: bool) {
        self.update_anchor(select);
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        self.caret = offset;
    }

    pub fn select_word_at(&mut self, offset: usize) {
        self.end_run();
        let offset = offset.min(self.text.len());
        let class = |c: char| match c {
            c if c.is_alphanumeric() || matches!(c, '_' | '-' | '/' | '#') => 0,
            c if c.is_whitespace() => 1,
            _ => 2,
        };
        let at = self.text[offset..].chars().next().or_else(|| self.text[..offset].chars().next_back());
        let Some(kind) = at.map(class) else { return };
        let before = self.text[..offset].chars().rev().take_while(|&c| class(c) == kind);
        let start = offset - before.map(char::len_utf8).sum::<usize>();
        let after = self.text[offset..].chars().take_while(|&c| class(c) == kind);
        let end = offset + after.map(char::len_utf8).sum::<usize>();
        self.anchor = Some(start);
        self.caret = end;
    }

    pub fn select_all(&mut self) {
        self.end_run();
        self.anchor = Some(0);
        self.caret = self.text.len();
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.selection().map(|(a, b)| &self.text[a..b])
    }

    pub fn take(&mut self) -> String {
        self.checkpoint(Kind::Other);
        self.caret = 0;
        self.anchor = None;
        std::mem::take(&mut self.text)
    }

    pub fn set(&mut self, text: impl Into<String>) {
        *self = Self::new(text);
    }

    pub fn trigger_query(&self, triggers: &[char]) -> Option<(char, &str)> {
        let start = self.word_start(self.caret);
        let word = &self.text[start..self.caret];
        let trigger = word.chars().next().filter(|c| triggers.contains(c))?;
        Some((trigger, &word[trigger.len_utf8()..]))
    }

    pub fn complete_word(&mut self, token: &str) {
        self.checkpoint(Kind::Other);
        let start = self.word_start(self.caret);
        self.text.replace_range(start..self.caret, &format!("{token} "));
        self.caret = start + token.len() + 1;
        self.anchor = None;
    }

    pub fn clear_word(&mut self) {
        self.checkpoint(Kind::Other);
        let start = self.word_start(self.caret);
        self.text.replace_range(start..self.caret, "");
        self.caret = start;
        self.anchor = None;
    }

    fn update_anchor(&mut self, select: bool) {
        self.end_run();
        if select {
            self.anchor.get_or_insert(self.caret);
        } else {
            self.anchor = None;
        }
    }

    fn delete_selection(&mut self) -> bool {
        let Some((a, b)) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.text.replace_range(a..b, "");
        self.caret = a;
        self.anchor = None;
        true
    }

    fn prev_boundary(&self, from: usize) -> Option<usize> {
        self.text[..from].char_indices().next_back().map(|(i, _)| i)
    }

    fn next_boundary(&self, from: usize) -> Option<usize> {
        self.text[from..].chars().next().map(|c| from + c.len_utf8())
    }

    fn word_start(&self, from: usize) -> usize {
        self.text[..from].rfind(char::is_whitespace).map_or(0, |i| i + 1)
    }
}

#[cfg(test)]
mod tests;
