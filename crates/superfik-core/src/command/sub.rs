//! The commands that take their own words: `/mcp`, `/memory` and
//! `/verification`.

use crate::MemoryId;

/// `/mcp`, `/mcp approve|deny|login|logout <name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpCommand {
    List,
    Approve(String),
    Deny(String),
    /// Sign in to a remote server in the browser.
    Login(String),
    Logout(String),
}

/// `/memory`, `/memory <search>`, `/memory optimize`, `/memory undo`,
/// `/memory forgotten`, `/memory restore <id>`, `/memory edit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryCommand {
    List,
    Search(String),
    /// An agent tidies the memory: merges, corrects, reclassifies, drops.
    Optimize,
    /// Puts back the memory as it was before the last optimization.
    Undo,
    Forgotten,
    Restore(MemoryId),
    /// The project's SUPERFIK.md, opened in the user's editor.
    Edit,
}

impl MemoryCommand {
    pub(super) fn parse(argument: Option<String>) -> Option<Self> {
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

/// `/verification`, `/verification off`, `/verification <criteria>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationCommand {
    Show,
    Clear,
    Set(String),
}

impl VerificationCommand {
    pub(super) fn parse(argument: Option<String>) -> Self {
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
