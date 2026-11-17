mod describing;
mod readers;
mod reading;
mod secrets;
mod sharing;
mod sign_in;
mod wildcards;

pub use sign_in::sign_in_refused;

use std::path::{Path, PathBuf};

use crate::destructive::words;
use crate::permission_rules::{hides_more, normalized, split_command, without_harmless_redirects};
use crate::shell::ShellCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandClass {
    ReadOnly,
    Forbidden(String),
    Risky(String),
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GitFacts<'a> {
    pub current_branch: Option<&'a str>,
    pub default_branch: Option<&'a str>,
}

pub type Readable<'a> = &'a dyn Fn(&Path) -> bool;

pub fn nowhere(_: &Path) -> bool {
    false
}

pub fn classify(command: &str, git: &GitFacts<'_>, readable: Readable<'_>) -> CommandClass {
    classify_command(&ShellCommand::parse(command), git, readable)
}

pub fn classify_command(command: &ShellCommand, git: &GitFacts<'_>, readable: Readable<'_>) -> CommandClass {
    let parts = command.commands();
    if let Some(why) = parts.iter().find_map(|part| secrets::exposes(part)) {
        return CommandClass::Forbidden(format!(
            "Refused: {why}. Never print secrets into the conversation; if the user needs it, they can run it themselves with `! <command>`."
        ));
    }
    let risk = parts.iter().find_map(|part| {
        let words = words(&normalized(part));
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        sharing::risk(&words, git)
    });
    if let Some(why) = risk {
        return CommandClass::Risky(format!("This needs the user's approval every time: {why}."));
    }
    if !command.flat() || parts.is_empty() {
        return CommandClass::Unknown;
    }
    if parts.iter().all(|part| reads_only(part, readable)) { CommandClass::ReadOnly } else { CommandClass::Unknown }
}

pub fn paths_read_anywhere(command: &ShellCommand, home: Option<&str>) -> Option<Vec<PathBuf>> {
    let parts = command.commands();
    if !command.flat() || parts.is_empty() {
        return None;
    }
    let mut paths = Vec::new();
    for part in parts {
        let part = reading::with_home(part, home);
        if !reads_only(&part, &|_| true) {
            return None;
        }
        let written = without_harmless_redirects(&part);
        for word in words(&written) {
            let value = word.split_once('=').map_or(word.as_str(), |(_, value)| value);
            paths.extend([word.as_str(), value].into_iter().filter(|path| path.starts_with('/')).map(PathBuf::from));
        }
    }
    paths.dedup();
    Some(paths)
}

pub fn only_reads(part: &str) -> bool {
    reads_only(part, &nowhere)
}

fn reads_only(part: &str, readable: Readable<'_>) -> bool {
    let written = without_harmless_redirects(part);
    let (_, balanced) = split_command(&written);
    let words = words(&written);
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    balanced
        && !hides_more(part)
        && (reading::only_reads(&written, &words, readable) || describing::only_describes(part))
}

#[cfg(test)]
mod properties;
#[cfg(test)]
mod tests;
