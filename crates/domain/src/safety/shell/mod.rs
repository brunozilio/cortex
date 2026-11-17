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
