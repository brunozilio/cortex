//! The terminal chat's own `/commands`: what each is called and how typed
//! text names one. Any other `/name` is a skill or custom command the
//! engine expands. Pure.

use crate::{Effort, PermissionMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    New,
    Model(Option<String>),
    Mode(Option<PermissionMode>),
    Effort(Option<Effort>),
    Compact,
    Memory(Option<String>),
    Skills,
    Agents,
    Cost,
    Exit,
}

impl Command {
    /// Every command with what it does, for `/help` and completion.
    pub const HELP: [(&'static str, &'static str); 11] = [
        ("help", "Show the commands and keys"),
        ("new", "Start a new conversation (also /clear)"),
        ("model", "Pick the model, or /model <id>"),
        ("mode", "Pick ask, edits, plan or bypass"),
        ("effort", "Set low, medium, high, xhigh or max"),
        ("compact", "Summarize the conversation to free context"),
        ("memory", "List the project's memory, or /memory <search>"),
        ("skills", "List skills and commands"),
        ("agents", "List agents"),
        ("cost", "Show this session's cost"),
        ("exit", "Quit (also /quit)"),
    ];

    /// `None` when `text` is not one of these (a skill, a command, or a
    /// path typed as a prompt).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().strip_prefix('/')?;
        let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let rest = rest.trim();
        let argument = (!rest.is_empty()).then(|| rest.to_owned());
        let bare = |command: Self| argument.is_none().then_some(command);
        match name.to_ascii_lowercase().as_str() {
            "help" | "?" => bare(Self::Help),
            "new" | "clear" => bare(Self::New),
            "model" => Some(Self::Model(argument)),
            "mode" => match &argument {
                None => Some(Self::Mode(None)),
                Some(mode) => mode_named(mode).map(|mode| Self::Mode(Some(mode))),
            },
            "effort" => match &argument {
                None => Some(Self::Effort(None)),
                Some(level) => level_named(level).map(|level| Self::Effort(Some(level))),
            },
            "compact" => bare(Self::Compact),
            "memory" => Some(Self::Memory(argument)),
            "skills" | "commands" => bare(Self::Skills),
            "agents" => bare(Self::Agents),
            "cost" => bare(Self::Cost),
            "exit" | "quit" => bare(Self::Exit),
            _ => None,
        }
    }
}

/// The short name a mode is typed and shown as (`/mode edits`).
pub const fn mode_label(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "ask",
        PermissionMode::AcceptEdits => "edits",
        PermissionMode::Plan => "plan",
        PermissionMode::Bypass => "bypass",
    }
}

pub fn mode_named(name: &str) -> Option<PermissionMode> {
    Some(match name.to_ascii_lowercase().as_str() {
        "ask" | "default" => PermissionMode::Ask,
        "edits" | "acceptedits" | "accept-edits" => PermissionMode::AcceptEdits,
        "plan" => PermissionMode::Plan,
        "bypass" | "yolo" => PermissionMode::Bypass,
        _ => return None,
    })
}

pub fn level_named(name: &str) -> Option<Effort> {
    Some(match name.to_ascii_lowercase().as_str() {
        "low" => Effort::Low,
        "medium" => Effort::Medium,
        "high" => Effort::High,
        "xhigh" => Effort::XHigh,
        "max" => Effort::Max,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_parse_with_their_arguments() {
        assert_eq!(Command::parse("  /CLEAR "), Some(Command::New));
        assert_eq!(Command::parse("/model"), Some(Command::Model(None)));
        assert_eq!(Command::parse("/model claude-opus-5-5"), Some(Command::Model(Some("claude-opus-5-5".into()))));
        assert_eq!(Command::parse("/mode plan"), Some(Command::Mode(Some(PermissionMode::Plan))));
        assert_eq!(Command::parse("/effort max"), Some(Command::Effort(Some(Effort::Max))));
        assert_eq!(Command::parse("/memory the renderer"), Some(Command::Memory(Some("the renderer".into()))));
    }

    #[test]
    fn anything_else_is_left_to_the_engine() {
        assert_eq!(Command::parse("/new explain this"), None, "a bare command takes no argument");
        assert_eq!(Command::parse("/mode sideways"), None);
        assert_eq!(Command::parse("/audit src"), None, "a custom command");
        assert_eq!(Command::parse("/etc/hosts"), None, "a path");
        assert_eq!(Command::parse("hello"), None);
    }

    #[test]
    fn modes_and_levels_read_back_from_their_names() {
        for mode in PermissionMode::ALL {
            assert_eq!(mode_named(mode_label(mode)), Some(mode));
        }
        for level in Effort::ALL {
            assert_eq!(level_named(level.cli_value()), Some(level));
        }
        let mut mode = PermissionMode::Ask;
        for expected in [PermissionMode::AcceptEdits, PermissionMode::Plan, PermissionMode::Bypass, PermissionMode::Ask]
        {
            mode = mode.next();
            assert_eq!(mode, expected);
        }
    }

    #[test]
    fn help_names_only_commands_that_parse() {
        for (name, _) in Command::HELP {
            assert!(Command::parse(&format!("/{name}")).is_some(), "{name}");
        }
    }
}
