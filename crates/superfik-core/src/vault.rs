//! Password vaults (Bitwarden) as agents use them: by name, never by value.
//! The agent names an item and a field and the variable that should hold
//! it; the backend asks the user, runs the command with the value in its
//! environment and hides the value in what the command prints. Pure.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultKind {
    Bitwarden,
}

impl VaultKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bitwarden => "Bitwarden",
        }
    }

    pub const fn unlock_secret(self) -> &'static str {
        match self {
            Self::Bitwarden => "password",
        }
    }
}

pub const VAULT_SERVER: &str = "superfik_vault";
pub const LIST_SECRETS_TOOL: &str = "list_secrets";
pub const RUN_WITH_SECRETS_TOOL: &str = "run_with_secrets";
pub const REQUEST_CREDENTIALS_TOOL: &str = "request_credentials";
pub const LIST_GRANTED_TOOL: &str = "list_granted_credentials";
pub const RELEASE_CREDENTIALS_TOOL: &str = "release_credentials";
pub const VAULT_TOOLS: [&str; 5] =
    [LIST_SECRETS_TOOL, RUN_WITH_SECRETS_TOOL, REQUEST_CREDENTIALS_TOOL, LIST_GRANTED_TOOL, RELEASE_CREDENTIALS_TOOL];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultTool {
    pub url: String,
    pub token: String,
}

impl VaultTool {
    pub fn allowed_tools() -> Vec<String> {
        VAULT_TOOLS.iter().map(|tool| format!("mcp__{VAULT_SERVER}__{tool}")).collect()
    }

    pub fn instructions(accounts: &[String]) -> String {
        format!(
            "{}: passwords and tokens through mcp__{VAULT_SERVER}__{LIST_SECRETS_TOOL} (names only) and \
             mcp__{VAULT_SERVER}__{RUN_WITH_SECRETS_TOOL} (runs a command with secrets in environment variables; \
             the user approves each use). You never see secret values: never ask for them, never try to print, \
             encode or send them anywhere the command does not need them.",
            accounts.join(", ")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretUse {
    pub item: String,
    pub field: String,
    pub env: String,
}

const RESERVED: &[&str] = &[
    "PATH",
    "HOME",
    "SHELL",
    "USER",
    "PWD",
    "IFS",
    "ENV",
    "BASH_ENV",
    "PATHEXT",
    "COMSPEC",
    "SYSTEMROOT",
    "WINDIR",
    "PSMODULEPATH",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMFILES",
];
const RESERVED_PREFIXES: &[&str] = &["LD_", "DYLD_", "BW_"];

pub fn env_problem(name: &str) -> Option<String> {
    let shaped = name.chars().next().is_some_and(|c| c.is_ascii_uppercase() || c == '_')
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !shaped {
        return Some(format!("`{name}` is not a valid variable name (use A-Z, 0-9 and _)"));
    }
    if RESERVED.contains(&name) || RESERVED_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
        return Some(format!("`{name}` cannot hold a secret"));
    }
    None
}

const REDACT_MIN: usize = 4;

pub fn redact(text: &str, secrets: &[(String, String)]) -> String {
    let mut values: Vec<(&str, &str)> = secrets
        .iter()
        .map(|(env, value)| (env.as_str(), value.trim()))
        .filter(|(_, value)| value.chars().count() >= REDACT_MIN)
        .collect();
    values.sort_by_key(|(_, value)| std::cmp::Reverse(value.len()));
    let mut out = text.to_owned();
    for (env, value) in values {
        out = out.replace(value, &format!("[secret ${env}]"));
    }
    out
}

pub fn ask_subject(command: &str, uses: &[SecretUse]) -> String {
    let secrets: Vec<String> = uses.iter().map(|u| format!("{} · {} → ${}", u.item, u.field, u.env)).collect();
    format!("{}\n$ {}", secrets.join("\n"), command.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(env: &str, value: &str) -> (String, String) {
        (env.to_owned(), value.to_owned())
    }

    #[test]
    fn variables_must_be_plain_and_harmless() {
        assert_eq!(env_problem("GH_TOKEN"), None);
        assert_eq!(env_problem("_X1"), None);
        assert!(env_problem("gh_token").is_some());
        assert!(env_problem("1X").is_some());
        assert!(env_problem("").is_some());
        assert!(env_problem("A-B").is_some());
        assert!(env_problem("PATH").is_some());
        assert!(env_problem("DYLD_INSERT_LIBRARIES").is_some());
        assert!(env_problem("BW_SESSION").is_some());
        assert!(env_problem("PATHEXT").is_some(), "Windows decides what runs from it");
        assert!(env_problem("COMSPEC").is_some());
    }

    #[test]
    fn values_are_hidden_longest_first() {
        let secrets = [secret("SHORT", "hunter"), secret("LONG", "hunter2-long"), secret("TINY", "ab")];
        assert_eq!(
            redact("a hunter2-long b hunter c ab", &secrets),
            "a [secret $LONG] b [secret $SHORT] c ab",
            "values under four characters stay"
        );
        assert_eq!(redact("token=abcd\n", &[secret("T", "abcd\n")]), "token=[secret $T]\n");
    }

    #[test]
    fn the_ask_names_every_secret_and_the_command() {
        let uses = [SecretUse { item: "GitHub".into(), field: "password".into(), env: "GH_TOKEN".into() }];
        assert_eq!(ask_subject(" gh auth status ", &uses), "GitHub · password → $GH_TOKEN\n$ gh auth status");
    }

    #[test]
    fn the_run_is_told_how_to_use_the_vault() {
        assert_eq!(
            VaultTool::allowed_tools(),
            [
                "mcp__superfik_vault__list_secrets",
                "mcp__superfik_vault__run_with_secrets",
                "mcp__superfik_vault__request_credentials",
                "mcp__superfik_vault__list_granted_credentials",
                "mcp__superfik_vault__release_credentials",
            ]
        );
        assert!(VaultTool::instructions(&["Bitwarden · Superfik".into()]).starts_with("Bitwarden · Superfik: "));
    }
}
