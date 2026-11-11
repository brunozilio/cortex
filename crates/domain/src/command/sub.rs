use crate::MemoryId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpCommand {
    List,
    Approve(String),
    Deny(String),
    Login(String),
    Logout(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryCommand {
    List,
    Search(String),
    Optimize,
    Undo,
    Forgotten,
    Restore(MemoryId),
    Edit,
}

impl MemoryCommand {
    pub fn parse(argument: Option<String>) -> Option<Self> {
        let Some(text) = argument else { return Some(Self::List) };
        let mut words = text.split_whitespace();
        let verb = words.next().unwrap_or_default().to_ascii_lowercase();
        let rest: Vec<&str> = words.collect();
        match (verb.as_str(), rest.as_slice()) {
            ("optimize" | "optimise", []) => Some(Self::Optimize),
            ("undo", []) => Some(Self::Undo),
            ("forgotten", []) => Some(Self::Forgotten),
            ("edit", []) => Some(Self::Edit),
            ("restore", [id]) => id.parse().ok().map(|id| Self::Restore(MemoryId(id))),
            ("restore", []) => None,
            _ => Some(Self::Search(text)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationCommand {
    Show,
    Clear,
    Set(String),
}

impl VerificationCommand {
    pub fn parse(argument: Option<String>) -> Self {
        match argument {
            None => Self::Show,
            Some(text) if text.eq_ignore_ascii_case("off") => Self::Clear,
            Some(text) => Self::Set(text),
        }
    }
}

impl McpCommand {
    pub(super) fn parse(argument: Option<&str>) -> Option<Self> {
        let Some(argument) = argument else { return Some(Self::List) };
        let mut words = argument.split_whitespace();
        let (verb, name) = (words.next()?.to_ascii_lowercase(), words.next()?.to_owned());
        if words.next().is_some() {
            return None;
        }
        match verb.as_str() {
            "approve" => Some(Self::Approve(name)),
            "deny" => Some(Self::Deny(name)),
            "login" => Some(Self::Login(name)),
            "logout" => Some(Self::Logout(name)),
            _ => None,
        }
    }
}
