use std::io::{Stdout, Write};

use ratatui::crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste, EnableFocusChange,
    EnableMouseCapture, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;

pub struct Modes<W: Write, R: FnMut()> {
    out: W,
    mouse: bool,
    keyboard: bool,
    restore: R,
}

impl Modes<Stdout, fn()> {
    pub fn enter(mouse: bool) -> Self {
        let keyboard = supports_keyboard_enhancement().unwrap_or(false);
        Modes::on(std::io::stdout(), mouse, keyboard, ratatui::restore)
    }
}

impl<W: Write, R: FnMut()> Modes<W, R> {
    pub fn on(mut out: W, mouse: bool, keyboard: bool, restore: R) -> Self {
        let _ = execute!(out, EnableBracketedPaste, EnableFocusChange);
        let mouse = mouse && execute!(out, EnableMouseCapture).is_ok();
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;
        let keyboard = keyboard && execute!(out, PushKeyboardEnhancementFlags(flags)).is_ok();
        Self { out, mouse, keyboard, restore }
    }
}

impl<W: Write, R: FnMut()> Drop for Modes<W, R> {
    fn drop(&mut self) {
        if self.keyboard {
            let _ = execute!(self.out, PopKeyboardEnhancementFlags);
        }
        let _ = execute!(self.out, DisableBracketedPaste, DisableFocusChange);
        if self.mouse {
            let _ = execute!(self.out, DisableMouseCapture);
        }
        (self.restore)();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io::Write;
    use std::rc::Rc;

    use super::Modes;

    #[derive(Clone, Default)]
    struct Written(Rc<RefCell<Vec<u8>>>);

    impl Written {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.borrow()).into_owned()
        }

        fn guard(&self, mouse: bool, keyboard: bool) -> Modes<Written, impl FnMut() + use<>> {
            let restored = self.clone();
            Modes::on(self.clone(), mouse, keyboard, move || restored.0.borrow_mut().extend(b"<restored>"))
        }
    }

    impl Write for Written {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn in_order(text: &str, wanted: &[&str]) -> bool {
        let mut rest = text;
        wanted.iter().all(|piece| match rest.find(piece) {
            Some(at) => {
                rest = &rest[at + piece.len()..];
                true
            }
            None => false,
        })
    }

    #[test]
    fn every_mode_turned_on_is_undone_before_the_terminal_is_restored() {
        let written = Written::default();
        let guard = written.guard(true, true);
        let entered = written.text();
        assert!(in_order(&entered, &["\x1b[?2004h", "\x1b[?1004h", "\x1b[?1000h", "\x1b[>1u"]), "{entered:?}");
        assert!(!entered.contains("<restored>"));
        drop(guard);
        let left = written.text()[entered.len()..].to_owned();
        assert!(in_order(&left, &["\x1b[<1u", "\x1b[?2004l", "\x1b[?1004l", "\x1b[?1000l", "<restored>"]), "{left:?}");
        assert!(left.ends_with("<restored>"), "{left:?}");
    }

    #[test]
    fn a_panic_while_the_guard_lives_still_restores_the_terminal() {
        let written = Written::default();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = written.guard(true, false);
            panic!("mid-frame");
        }));
        assert!(unwound.is_err());
        let text = written.text();
        assert!(in_order(&text, &["\x1b[?2004l", "\x1b[?1000l", "<restored>"]), "{text:?}");
    }

    #[test]
    fn modes_that_were_not_turned_on_are_not_turned_off() {
        let written = Written::default();
        drop(written.guard(false, false));
        let text = written.text();
        assert!(!text.contains("\x1b[?1000") && !text.contains("\x1b[<1u"), "{text:?}");
        assert!(text.ends_with("<restored>"), "{text:?}");
    }
}
