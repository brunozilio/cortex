use std::ops::Range;

pub(super) struct Opaque;

pub(super) type Parse<T> = Result<T, Opaque>;

const DEEPEST: usize = 48;

const RESERVED: [&str; 20] = [
    "if", "then", "elif", "else", "fi", "do", "done", "while", "until", "for", "in", "case", "esac", "function",
    "select", "coproc", "{", "}", "!", "[[",
];

const CLOSING: [&str; 9] = ["then", "elif", "else", "fi", "do", "done", "esac", "}", "in"];

pub(super) struct Heredoc {
    pub(super) delimiter: Vec<u8>,
    pub(super) strip_tabs: bool,
    pub(super) expands: bool,
}

pub(super) struct Parser<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) at: usize,
    pub(super) end: usize,
    pub(super) commands: Vec<Range<usize>>,
    pub(super) flat: bool,
    pub(super) heredocs: Vec<Heredoc>,
    pub(super) depth: usize,
}

pub(super) fn parsed(text: &str) -> Option<(Vec<Range<usize>>, bool)> {
    let mut parser = Parser::over(text.as_bytes(), 0, text.len(), 0);
    parser.whole().ok()?;
    Some((parser.commands, parser.flat))
}

pub(super) fn blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}

pub(super) fn delimiter(byte: u8) -> bool {
    blank(byte) || matches!(byte, b'\n' | b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>')
}

impl<'a> Parser<'a> {
    pub(super) fn over(bytes: &'a [u8], at: usize, end: usize, depth: usize) -> Self {
        Self { bytes, at, end, commands: Vec::new(), flat: true, heredocs: Vec::new(), depth }
    }

    pub(super) fn whole(&mut self) -> Parse<()> {
        self.list(&[])?;
        if self.at < self.end || !self.heredocs.is_empty() {
            return Err(Opaque);
        }
        Ok(())
    }

    pub(super) fn peek(&self) -> Option<u8> {
        (self.at < self.end).then(|| self.bytes[self.at])
    }

    pub(super) fn starts(&self, text: &str) -> bool {
        self.bytes.get(self.at..self.end).is_some_and(|rest| rest.starts_with(text.as_bytes()))
    }

    pub(super) fn skip_blanks(&mut self) {
        while let Some(byte) = self.peek() {
            if blank(byte) {
                self.at += 1;
            } else if self.starts("\\\n") {
                self.at += 2;
            } else if byte == b'#' {
                while self.peek().is_some_and(|byte| byte != b'\n') {
                    self.at += 1;
                }
            } else {
                return;
            }
        }
    }

    fn skip_newlines(&mut self) -> Parse<()> {
        loop {
            self.skip_blanks();
            if self.peek() != Some(b'\n') {
                return Ok(());
            }
            self.newline()?;
        }
    }

    pub(super) fn keyword(&self) -> Option<&'static str> {
        RESERVED.into_iter().find(|word| {
            let after = self.bytes.get(self.at + word.len()).copied().filter(|_| self.at + word.len() < self.end);
            let ends = match *word {
                "{" | "}" | "!" | "[[" => {
                    after.is_none_or(|byte| blank(byte) || matches!(byte, b'\n' | b';' | b'&' | b'|' | b')'))
                }
                _ => after.is_none_or(delimiter),
            };
            self.starts(word) && ends
        })
    }

    fn stops(&self, stop: &[&str]) -> bool {
        match self.peek() {
            None => true,
            Some(b')') => stop.contains(&")"),
            Some(b';') => stop.contains(&";;") && (self.starts(";;") || self.starts(";&")),
            Some(_) => self.keyword().is_some_and(|word| stop.contains(&word)),
        }
    }

    pub(super) fn list(&mut self, stop: &[&str]) -> Parse<()> {
        loop {
            self.skip_newlines()?;
            if self.stops(stop) {
                return Ok(());
            }
            self.and_or()?;
            self.skip_blanks();
            match self.peek() {
                Some(b';') if !self.starts(";;") && !self.starts(";&") => self.at += 1,
                Some(b'&') => self.at += 1,
                Some(b'\n') => self.newline()?,
                _ if self.stops(stop) => return Ok(()),
                _ => return Err(Opaque),
            }
        }
    }

    fn and_or(&mut self) -> Parse<()> {
        self.pipeline()?;
        loop {
            self.skip_blanks();
            if !self.starts("&&") && !self.starts("||") {
                return Ok(());
            }
            self.at += 2;
            self.skip_newlines()?;
            self.pipeline()?;
        }
    }

    fn pipeline(&mut self) -> Parse<()> {
        self.skip_blanks();
        if self.keyword() == Some("!") {
            self.flat = false;
            self.at += 1;
        }
        self.command()?;
        loop {
            self.skip_blanks();
            if self.peek() != Some(b'|') || self.starts("||") {
                return Ok(());
            }
            self.at += 1;
            if self.peek() == Some(b'&') {
                self.at += 1;
            }
            self.skip_newlines()?;
            self.command()?;
        }
    }

    pub(super) fn command(&mut self) -> Parse<()> {
        self.depth += 1;
        if self.depth > DEEPEST {
            return Err(Opaque);
        }
        let parsed = self.command_here();
        self.depth -= 1;
        parsed
    }

    fn command_here(&mut self) -> Parse<()> {
        self.skip_blanks();
        if self.starts("((") {
            return Err(Opaque);
        }
        if self.peek() == Some(b'(') {
            self.flat = false;
            self.at += 1;
            self.list(&[")"])?;
            self.closing(b')')?;
            return self.redirects();
        }
        match self.keyword() {
            Some("{") => self.group(),
            Some("if") => self.if_clause(),
            Some(word @ ("while" | "until")) => {
                self.flat = false;
                self.at += word.len();
                self.list(&["do"])?;
                self.do_group()
            }
            Some(word @ ("for" | "select")) => self.for_clause(word),
            Some("case") => self.case_clause(),
            Some("function") => self.function(),
            Some("[[") => self.test_clause(),
            Some(word) if word == "coproc" || CLOSING.contains(&word) => Err(Opaque),
            _ if self.defines_function() => self.named_function(),
            _ => self.simple(),
        }
    }

    pub(super) fn closing(&mut self, byte: u8) -> Parse<()> {
        self.skip_blanks();
        if self.peek() != Some(byte) {
            return Err(Opaque);
        }
        self.at += 1;
        Ok(())
    }

    fn expect(&mut self, word: &str) -> Parse<()> {
        self.skip_blanks();
        if self.keyword().is_none_or(|found| found != word) {
            return Err(Opaque);
        }
        self.at += word.len();
        Ok(())
    }

    fn group(&mut self) -> Parse<()> {
        self.flat = false;
        self.at += 1;
        self.list(&["}"])?;
        self.expect("}")?;
        self.redirects()
    }

    fn do_group(&mut self) -> Parse<()> {
        self.expect("do")?;
        self.list(&["done"])?;
        self.expect("done")?;
        self.redirects()
    }

    fn if_clause(&mut self) -> Parse<()> {
        self.flat = false;
        self.at += 2;
        self.list(&["then"])?;
        self.expect("then")?;
        self.list(&["elif", "else", "fi"])?;
        loop {
            match self.keyword() {
                Some("elif") => {
                    self.at += 4;
                    self.list(&["then"])?;
                    self.expect("then")?;
                    self.list(&["elif", "else", "fi"])?;
                }
                Some("else") => {
                    self.at += 4;
                    self.list(&["fi"])?;
                }
                Some("fi") => {
                    self.at += 2;
                    return self.redirects();
                }
                _ => return Err(Opaque),
            }
        }
    }

    fn for_clause(&mut self, word: &str) -> Parse<()> {
        self.flat = false;
        self.at += word.len();
        self.skip_blanks();
        if self.starts("((") {
            return Err(Opaque);
        }
        self.word()?;
        self.skip_newlines()?;
        if self.keyword() == Some("in") {
            self.at += 2;
            self.items()?;
        } else if self.peek() == Some(b';') {
            self.at += 1;
        }
        self.skip_newlines()?;
        self.do_group()
    }

    fn items(&mut self) -> Parse<()> {
        loop {
            self.skip_blanks();
            match self.peek() {
                Some(b';') => {
                    self.at += 1;
                    return Ok(());
                }
                Some(b'\n') => return self.newline(),
                Some(byte) if !delimiter(byte) => self.word()?,
                _ => return Err(Opaque),
            }
        }
    }

    fn case_clause(&mut self) -> Parse<()> {
        self.flat = false;
        self.at += 4;
        self.skip_blanks();
        self.word()?;
        self.skip_newlines()?;
        self.expect("in")?;
        loop {
            self.skip_newlines()?;
            if self.keyword() == Some("esac") {
                self.at += 4;
                return self.redirects();
            }
            self.patterns()?;
            self.list(&[";;", "esac"])?;
            if self.starts(";;&") {
                self.at += 3;
            } else if self.starts(";;") || self.starts(";&") {
                self.at += 2;
            } else if self.keyword() != Some("esac") {
                return Err(Opaque);
            }
        }
    }

    fn patterns(&mut self) -> Parse<()> {
        if self.peek() == Some(b'(') {
            self.at += 1;
        }
        loop {
            self.skip_blanks();
            self.word()?;
            self.skip_blanks();
            match self.peek() {
                Some(b'|') => self.at += 1,
                Some(b')') => {
                    self.at += 1;
                    return Ok(());
                }
                _ => return Err(Opaque),
            }
        }
    }

    fn function(&mut self) -> Parse<()> {
        self.flat = false;
        self.at += "function".len();
        self.skip_blanks();
        self.word()?;
        self.skip_blanks();
        if self.peek() == Some(b'(') {
            self.at += 1;
            self.closing(b')')?;
        }
        self.function_body()
    }

    fn named_function(&mut self) -> Parse<()> {
        while self.peek().is_some_and(|byte| !delimiter(byte)) {
            self.at += 1;
        }
        self.closing(b'(')?;
        self.closing(b')')?;
        self.function_body()
    }

    fn defines_function(&self) -> bool {
        let name = self.bytes[self.at..self.end]
            .iter()
            .take_while(|byte| !delimiter(**byte) && !matches!(byte, b'\'' | b'"' | b'$' | b'`' | b'\\'))
            .count();
        let rest = &self.bytes[self.at + name..self.end];
        let mut after = rest.iter().skip_while(|byte| blank(**byte));
        name > 0 && after.next() == Some(&b'(') && after.find(|byte| !blank(**byte)) == Some(&b')')
    }

    fn function_body(&mut self) -> Parse<()> {
        self.flat = false;
        self.skip_newlines()?;
        self.command()
    }

    fn test_clause(&mut self) -> Parse<()> {
        self.flat = false;
        let start = self.at;
        self.at += 2;
        loop {
            self.skip_blanks();
            match self.peek() {
                None => return Err(Opaque),
                Some(b'\n') => self.newline()?,
                Some(b']') if self.starts("]]") && self.bytes.get(self.at + 2).is_none_or(|byte| delimiter(*byte)) => {
                    self.at += 2;
                    self.commands.push(start..self.at);
                    return self.redirects();
                }
                Some(byte) if delimiter(byte) && !self.starts("<(") && !self.starts(">(") => self.at += 1,
                Some(_) => self.word()?,
            }
        }
    }
}
