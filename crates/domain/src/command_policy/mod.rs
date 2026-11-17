mod describing;
mod readers;
mod reading;
mod secrets;
mod sharing;
mod sign_in;
mod wildcards;

pub use sign_in::sign_in_refused;

use crate::destructive::words;
use crate::permission_rules::{hides_more, normalized, split_command, without_harmless_redirects};

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

pub fn classify(command: &str, git: &GitFacts<'_>) -> CommandClass {
    let (parts, balanced) = split_command(command);
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
    if !balanced || parts.is_empty() {
        return CommandClass::Unknown;
    }
    if parts.iter().all(|part| only_reads(part)) { CommandClass::ReadOnly } else { CommandClass::Unknown }
}

pub fn only_reads(part: &str) -> bool {
    let written = without_harmless_redirects(part);
    let (_, balanced) = split_command(&written);
    let words = words(&written);
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    balanced && !hides_more(part) && (reading::only_reads(&written, &words) || describing::only_describes(part))
}

#[cfg(test)]
mod tests;
