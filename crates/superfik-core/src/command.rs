//! The `/commands` of the composer: what each one is called and how typed
//! text names one. Pure; what a command does lives in the app's reducer.

use crate::Effort;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    New,
    Clear,
    Context,
    Memory,
    Notes,
    Skills,
    Plugins,
    Commands,
    Repository,
    Model,
    Compact,
    Mode,
    Upload,
    Effort(Option<Effort>),
    Browser,
    Share,
    Publish,
    Voice,
    Btw,
    Rename,
}

impl Command {
    pub const ALL: [Self; 20] = [
        Self::Context,
        Self::Memory,
        Self::Notes,
        Self::Skills,
        Self::Plugins,
        Self::Commands,
        Self::Repository,
        Self::Model,
        Self::Compact,
        Self::Mode,
        Self::Upload,
        Self::Effort(None),
        Self::Browser,
        Self::Share,
        Self::Publish,
        Self::Voice,
        Self::Btw,
        Self::Rename,
        Self::New,
        Self::Clear,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Clear => "clear",
            Self::Context => "context",
            Self::Memory => "memory",
            Self::Notes => "notes",
            Self::Skills => "skills",
            Self::Plugins => "plugins",
            Self::Commands => "commands",
            Self::Repository => "repository",
            Self::Model => "model",
            Self::Compact => "compact",
            Self::Mode => "mode",
            Self::Upload => "upload",
            Self::Effort(_) => "effort",
            Self::Browser => "browser",
            Self::Share => "share",
            Self::Publish => "publish",
            Self::Voice => "voice",
            Self::Btw => "btw",
            Self::Rename => "rename",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().strip_prefix('/')?.to_ascii_lowercase();
        let mut words = text.split_whitespace();
        let name = words.next()?;
        if name == "effort" {
            return match (words.next(), words.next()) {
                (None, _) => Some(Self::Effort(None)),
                (Some(level), None) => level_named(level).map(|l| Self::Effort(Some(l))),
                _ => None,
            };
        }
        if words.next().is_some() {
            return None;
        }
        Some(match name {
            "new" => Self::New,
            "clear" => Self::Clear,
            "context" => Self::Context,
            "memory" => Self::Memory,
            "notes" => Self::Notes,
            "skills" => Self::Skills,
            "plugins" => Self::Plugins,
            "commands" => Self::Commands,
            "repository" => Self::Repository,
            "model" => Self::Model,
            "compact" => Self::Compact,
            "mode" => Self::Mode,
            "upload" => Self::Upload,
            "browser" => Self::Browser,
            "share" => Self::Share,
            "publish" => Self::Publish,
            "voice" => Self::Voice,
            "btw" => Self::Btw,
            "rename" => Self::Rename,
            _ => return None,
        })
    }
}

pub fn argument<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let rest = text.trim().strip_prefix('/')?.strip_prefix(name)?;
    let argument = rest.trim();
    (rest.starts_with(char::is_whitespace) && !argument.is_empty()).then_some(argument)
}

fn level_named(name: &str) -> Option<Effort> {
    Some(match name {
        "low" => Effort::Low,
        "medium" => Effort::Medium,
        "high" => Effort::High,
        "xhigh" => Effort::XHigh,
        "max" | "ultracode" => Effort::Max,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_commands_accept_only_the_command_itself() {
        for command in [Command::New, Command::Clear] {
            assert!(Command::ALL.contains(&command));
            assert_eq!(Command::parse(&format!("  /{}  ", command.name().to_uppercase())), Some(command));
            assert_eq!(Command::parse(&format!("/{} explain this", command.name())), None);
        }
    }

    #[test]
    fn an_argument_is_what_follows_the_name() {
        assert_eq!(argument("/share Design Ops ", "share"), Some("Design Ops"));
        assert_eq!(argument("/share", "share"), None);
        assert_eq!(argument("/shared x", "share"), None);
        assert_eq!(argument("share x", "share"), None);
        assert_eq!(Command::parse("/share"), Some(Command::Share));
    }
}
