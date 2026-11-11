//! `/vim`: the prompt edited with vim's keys. Esc goes to normal mode
//! (`h l w b 0 $ j k x dd D i a I A o`), `i`/`a`/… back to inserting;
//! Enter sends from either.

use ratatui::crossterm::event::KeyCode;

use super::input::Input;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Vim {
    /// In normal mode (keys move and edit) rather than inserting.
    pub normal: bool,
    /// The first key of a two-key command (`d` of `dd`).
    pending: Option<char>,
}

impl Vim {
    /// Esc while inserting: to normal mode, the cursor back one like vim.
    pub fn escape(&mut self, input: &mut Input) {
        if !self.normal {
            self.normal = true;
            input.left();
        }
        self.pending = None;
    }

    /// A key in normal mode; `false` when it is not vim's (Enter, Ctrl…).
    pub fn key(&mut self, code: KeyCode, input: &mut Input) -> bool {
        let KeyCode::Char(c) = code else {
            match code {
                KeyCode::Left | KeyCode::Backspace => input.left(),
                KeyCode::Right => input.right(),
                _ => return false,
            }
            return true;
        };
        if let Some(first) = self.pending.take() {
            match (first, c) {
                ('d', 'd') => input.clear(),
                ('d', 'w') => input.delete_word_forward(),
                _ => {}
            }
            return true;
        }
        match c {
            'h' => input.left(),
            'l' => input.right(),
            'w' => {
                input.word_right();
                while input.text()[input.cursor()..].starts_with(char::is_whitespace) {
                    input.right();
                }
            }
            'b' => input.word_left(),
            '0' | '^' => input.home(),
            '$' => input.end(),
            'j' => input.line(false),
            'k' => input.line(true),
            'x' => input.delete(),
            'D' => {
                let start = input.cursor();
                input.end();
                while input.cursor() > start {
                    input.backspace();
                }
            }
            'd' => self.pending = Some('d'),
            'i' => self.normal = false,
            'a' => {
                input.right();
                self.normal = false;
            }
            'I' => {
                input.home();
                self.normal = false;
            }
            'A' => {
                input.end();
                self.normal = false;
            }
            'o' => {
                input.end();
                input.insert('\n');
                self.normal = false;
            }
            _ => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(vim: &mut Vim, input: &mut Input, keys: &str) {
        for c in keys.chars() {
            vim.key(KeyCode::Char(c), input);
        }
    }

    #[test]
    fn normal_mode_moves_edits_and_goes_back_to_inserting() {
        let (mut vim, mut input) = (Vim::default(), Input::default());
        input.set("fix the tests");
        vim.escape(&mut input);
        assert!(vim.normal);
        typed(&mut vim, &mut input, "0wD");
        assert_eq!(input.text(), "fix ");
        typed(&mut vim, &mut input, "A");
        assert!(!vim.normal, "A inserts at the end");
        for c in "docs".chars() {
            input.insert(c);
        }
        assert_eq!(input.text(), "fix docs");
        vim.escape(&mut input);
        typed(&mut vim, &mut input, "0x");
        assert_eq!(input.text(), "ix docs");
        typed(&mut vim, &mut input, "dd");
        assert!(input.is_empty());
    }
}
