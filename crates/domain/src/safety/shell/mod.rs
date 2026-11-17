mod derived;
mod parser;
mod rough;
mod words;

#[cfg(test)]
pub(crate) mod sample;
#[cfg(test)]
mod tests;

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand {
    text: String,
    commands: Vec<Range<usize>>,
    flat: bool,
    opaque: bool,
}

impl ShellCommand {
    pub fn parse(text: &str) -> Self {
        let (commands, flat, opaque) = ranges(text);
        Self { text: text.to_owned(), commands, flat, opaque }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn commands(&self) -> Vec<&str> {
        slices(&self.text, &self.commands)
    }

    pub fn opaque(&self) -> bool {
        self.opaque
    }

    pub fn flat(&self) -> bool {
        self.flat && !self.opaque
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirection {
    pub operator: &'static str,
    pub target: String,
}

impl Redirection {
    pub fn reads_a_file(&self) -> bool {
        matches!(self.operator, "<" | "<>")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimpleCommand {
    pub words: Vec<String>,
    pub redirections: Vec<Redirection>,
}

impl SimpleCommand {
    pub fn of(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let mut parser = parser::Parser::over(bytes, 0, text.len(), 0);
        let lexed = parser.lexed().ok()?;
        let piece = |range: Range<usize>| text.get(range).map(str::to_owned);
        let words = lexed.words.into_iter().map(piece).collect::<Option<Vec<_>>>()?;
        let redirections = lexed
            .redirections
            .into_iter()
            .map(|(operator, target)| piece(target).map(|target| Redirection { operator, target }))
            .collect::<Option<Vec<_>>>()?;
        Some(Self { words, redirections })
    }

    pub fn bare(text: &str) -> String {
        match Self::of(text) {
            Some(simple) if !simple.redirections.is_empty() => simple.words.join(" "),
            _ => text.to_owned(),
        }
    }
}

pub fn simple_commands(text: &str) -> (Vec<&str>, bool) {
    let (commands, _, opaque) = ranges(text);
    (slices(text, &commands), !opaque)
}

fn ranges(text: &str) -> (Vec<Range<usize>>, bool, bool) {
    match parser::parsed(text) {
        Some((mut commands, flat)) => {
            commands.sort_by_key(|range| range.start);
            (commands, flat, false)
        }
        None => (rough::split(text), false, true),
    }
}

fn slices<'a>(text: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
    ranges
        .iter()
        .filter_map(|range| text.get(range.clone()))
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .collect()
}
