use std::ops::Range;

use super::parser::{Heredoc, Opaque, Parse, Parser, delimiter};

const REDIRECTS: [&str; 12] = ["&>>", "&>", ">>", ">&", ">|", "<<<", "<<-", "<<", "<&", "<>", ">", "<"];

const RUNS_IN_A_HEREDOC: [&str; 3] = ["$(", "`", "${"];

#[derive(Debug, Default)]
pub(super) struct Lexed {
    pub(super) words: Vec<Range<usize>>,
    pub(super) redirections: Vec<(&'static str, Range<usize>)>,
}

fn holds(bytes: &[u8], needle: &str) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle.as_bytes())
}

impl Parser<'_> {
    pub(super) fn simple(&mut self) -> Parse<()> {
        let start = self.at;
        let mut words: Vec<Range<usize>> = Vec::new();
        let mut last = start;
        loop {
            self.skip_blanks();
            match self.peek() {
                None | Some(b';' | b'\n' | b')' | b'|') => break,
                Some(b'&') if !self.starts("&>") => break,
                Some(b'(') => return Err(Opaque),
                _ => {}
            }
            if self.redirect()?.is_none() {
                let word = self.at;
                self.word()?;
                words.push(word..self.at);
            }
            last = self.at;
        }
        if last == start {
            return Err(Opaque);
        }
        self.commands.push(start..last);
        let derived = super::derived::commands(self.bytes, &words);
        self.commands.extend(derived);
        Ok(())
    }

    pub(super) fn redirects(&mut self) -> Parse<()> {
        loop {
            self.skip_blanks();
            if self.redirect()?.is_none() {
                return Ok(());
            }
        }
    }

    pub(super) fn lexed(&mut self) -> Parse<Lexed> {
        let mut lexed = Lexed::default();
        loop {
            self.skip_blanks();
            if self.peek().is_none() {
                return Ok(lexed);
            }
            match self.redirect()? {
                Some(redirection) => lexed.redirections.push(redirection),
                None => {
                    let word = self.at;
                    self.word()?;
                    lexed.words.push(word..self.at);
                }
            }
        }
    }

    fn redirect(&mut self) -> Parse<Option<(&'static str, Range<usize>)>> {
        if self.starts("<(") || self.starts(">(") {
            return Ok(None);
        }
        let digits = self.bytes[self.at..self.end].iter().take_while(|byte| byte.is_ascii_digit()).count();
        let rest = &self.bytes[self.at + digits..self.end];
        let Some(operator) = REDIRECTS.into_iter().find(|operator| rest.starts_with(operator.as_bytes())) else {
            return Ok(None);
        };
        self.at += digits + operator.len();
        self.skip_blanks();
        if self.peek().is_none_or(delimiter) {
            return Err(Opaque);
        }
        let target = self.at;
        self.word()?;
        if operator == "<<" || operator == "<<-" {
            self.heredocs.push(heredoc(&self.bytes[target..self.at], operator == "<<-"));
        }
        Ok(Some((operator, target..self.at)))
    }

    pub(super) fn word(&mut self) -> Parse<()> {
        let start = self.at;
        if self.starts("<(") || self.starts(">(") {
            self.flat = false;
            self.at += 2;
            self.nested()?;
        }
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => self.at = (self.at + 2).min(self.end),
                b'\'' => self.single_quoted()?,
                b'"' => {
                    self.at += 1;
                    self.double_quoted()?;
                }
                b'`' => self.backticks()?,
                b'$' => self.dollar()?,
                _ if delimiter(byte) => break,
                _ => self.at += 1,
            }
        }
        if self.at == start { Err(Opaque) } else { Ok(()) }
    }

    fn single_quoted(&mut self) -> Parse<()> {
        let close = self.bytes[self.at + 1..self.end].iter().position(|byte| *byte == b'\'').ok_or(Opaque)?;
        self.at += close + 2;
        Ok(())
    }

    fn double_quoted(&mut self) -> Parse<()> {
        loop {
            match self.peek() {
                None => return Err(Opaque),
                Some(b'"') => {
                    self.at += 1;
                    return Ok(());
                }
                Some(b'\\') => self.at = (self.at + 2).min(self.end),
                Some(b'`') => self.backticks()?,
                Some(b'$') => self.dollar()?,
                Some(_) => self.at += 1,
            }
        }
    }

    fn ansi_quoted(&mut self) -> Parse<()> {
        loop {
            match self.peek() {
                None => return Err(Opaque),
                Some(b'\'') => {
                    self.at += 1;
                    return Ok(());
                }
                Some(b'\\') => self.at = (self.at + 2).min(self.end),
                Some(_) => self.at += 1,
            }
        }
    }

    fn dollar(&mut self) -> Parse<()> {
        if self.starts("$((") {
            self.flat = false;
            self.at += 1;
            return self.balanced(b'(', b')');
        }
        if self.starts("$(") {
            self.flat = false;
            self.at += 2;
            return self.nested();
        }
        if self.starts("${") {
            self.at += 1;
            return self.balanced(b'{', b'}');
        }
        if self.starts("$'") {
            self.at += 2;
            return self.ansi_quoted();
        }
        if self.starts("$\"") {
            self.at += 2;
            return self.double_quoted();
        }
        self.at += 1;
        Ok(())
    }

    fn balanced(&mut self, open: u8, close: u8) -> Parse<()> {
        let start = self.at;
        let mut depth = 0usize;
        while let Some(byte) = self.peek() {
            self.at += 1;
            if byte == open {
                depth += 1;
            } else if byte == close {
                depth -= 1;
                if depth == 0 {
                    let inside = &self.bytes[start..self.at];
                    return if holds(inside, "$(") || holds(inside, "`") { Err(Opaque) } else { Ok(()) };
                }
            }
        }
        Err(Opaque)
    }

    fn nested(&mut self) -> Parse<()> {
        self.list(&[")"])?;
        self.closing(b')')
    }

    fn backticks(&mut self) -> Parse<()> {
        let open = self.at;
        let mut at = open + 1;
        let close = loop {
            match self.bytes.get(at).filter(|_| at < self.end) {
                None => return Err(Opaque),
                Some(b'\\') => at += 2,
                Some(b'`') => break at,
                Some(_) => at += 1,
            }
        };
        if holds(&self.bytes[open + 1..close], "\\") {
            return Err(Opaque);
        }
        self.flat = false;
        let mut inner = Parser::over(self.bytes, open + 1, close, self.depth + 1);
        inner.whole()?;
        self.commands.extend(inner.commands);
        self.at = close + 1;
        Ok(())
    }

    pub(super) fn newline(&mut self) -> Parse<()> {
        self.at += 1;
        for heredoc in std::mem::take(&mut self.heredocs) {
            self.body(&heredoc)?;
        }
        Ok(())
    }

    fn body(&mut self, heredoc: &Heredoc) -> Parse<()> {
        let start = self.at;
        while self.at < self.end {
            let line_end = self.bytes[self.at..self.end].iter().position(|byte| *byte == b'\n').map(|at| self.at + at);
            let line_end = line_end.unwrap_or(self.end);
            let line = &self.bytes[self.at..line_end];
            let line = if heredoc.strip_tabs { trim_tabs(line) } else { line };
            if line == heredoc.delimiter.as_slice() {
                let body = &self.bytes[start..self.at];
                if heredoc.expands && RUNS_IN_A_HEREDOC.iter().any(|mark| holds(body, mark)) {
                    return Err(Opaque);
                }
                self.at = (line_end + 1).min(self.end);
                return Ok(());
            }
            self.at = line_end + 1;
        }
        Err(Opaque)
    }
}

fn trim_tabs(line: &[u8]) -> &[u8] {
    let tabs = line.iter().take_while(|byte| **byte == b'\t').count();
    &line[tabs..]
}

fn heredoc(word: &[u8], strip_tabs: bool) -> Heredoc {
    let expands = !word.iter().any(|byte| matches!(byte, b'\'' | b'"' | b'\\'));
    let delimiter = word.iter().copied().filter(|byte| !matches!(byte, b'\'' | b'"' | b'\\')).collect();
    Heredoc { delimiter, strip_tabs, expands }
}
