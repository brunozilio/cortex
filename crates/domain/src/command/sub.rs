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
    Share(MemoryId),
    Edit,
    Seed,
    SeedKeep(Vec<usize>),
    Score,
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
            ("score", []) => Some(Self::Score),
            ("edit", []) => Some(Self::Edit),
            ("restore", [id]) => id.parse().ok().map(|id| Self::Restore(MemoryId(id))),
            ("share", [id]) => id.parse().ok().map(|id| Self::Share(MemoryId(id))),
            ("restore", []) => None,
            ("seed", []) => Some(Self::Seed),
            ("seed", ["keep", picked @ ..]) => {
                let picked: Option<Vec<usize>> = picked.iter().map(|n| n.parse().ok()).collect();
                picked.filter(|picked| !picked.is_empty()).map(Self::SeedKeep)
            }
            ("seed", _) => None,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleChange {
    Pick,
    Set(String),
    Clear,
}

pub fn model_command(argument: Option<String>) -> super::Command {
    use crate::model_roles::ModelRole;
    let Some(argument) = argument else { return super::Command::Model(None) };
    let (first, rest) = argument.split_once(char::is_whitespace).unwrap_or((argument.as_str(), ""));
    let rest = rest.trim();
    if first.eq_ignore_ascii_case("roles") && rest.is_empty() {
        return super::Command::ModelRoles;
    }
    let change = match rest {
        "" => RoleChange::Pick,
        clear if ["default", "none", "off"].iter().any(|word| clear.eq_ignore_ascii_case(word)) => RoleChange::Clear,
        model => RoleChange::Set(model.to_owned()),
    };
    if first.eq_ignore_ascii_case("fallbacks") {
        return super::Command::ModelFallbacks(change);
    }
    match ModelRole::parse(first) {
        Some(role) => super::Command::ModelRole(role, change),
        None => super::Command::Model(Some(argument)),
    }
}

pub fn accounts_command(argument: Option<&str>) -> Option<super::Command> {
    use crate::account_pool::Balance;
    let Some(argument) = argument else { return Some(super::Command::Accounts { removing: false }) };
    let (first, rest) = argument.split_once(char::is_whitespace).unwrap_or((argument, ""));
    let rest = rest.trim();
    match first.to_ascii_lowercase().as_str() {
        "remove" if rest.is_empty() => Some(super::Command::Accounts { removing: true }),
        "balance" if rest.is_empty() => Some(super::Command::AccountsBalance(None)),
        "balance" => Balance::parse(rest).map(|balance| super::Command::AccountsBalance(Some(balance))),
        _ => None,
    }
}
