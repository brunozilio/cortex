use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Refusal, Trust};

pub const LEGACY_OPT_IN: &str = "trustProjectHooks";
pub const NOT_TRUSTED: &str = "folder not trusted (/trust)";

pub fn legacy_notice() -> String {
    format!(
        "\"{LEGACY_OPT_IN}\" in your settings no longer does anything: a project's hooks run once you trust its \
         folder (/trust in the chat, --trust with --print). Remove the key."
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionKind {
    SettingsHook,
    HookScript,
    LocalSettings,
    McpServer,
    DeclaredProvider,
    ScriptTool,
    SkillScript,
    LspCommand,
    AgentPrivilege,
}

impl ContributionKind {
    pub const ALL: [Self; 9] = [
        Self::SettingsHook,
        Self::HookScript,
        Self::LocalSettings,
        Self::McpServer,
        Self::DeclaredProvider,
        Self::ScriptTool,
        Self::SkillScript,
        Self::LspCommand,
        Self::AgentPrivilege,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::SettingsHook => "Hooks in settings",
            Self::HookScript => "Hook scripts",
            Self::LocalSettings => "Grants in settings.local.json",
            Self::McpServer => "MCP servers",
            Self::DeclaredProvider => "Model providers",
            Self::ScriptTool => "Script tools",
            Self::SkillScript => "Skills and commands that run a script",
            Self::LspCommand => "Language server commands",
            Self::AgentPrivilege => "Agent definitions with hooks, MCP servers, a mode or a first prompt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contribution {
    pub kind: ContributionKind,
    pub what: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Contributions(Vec<Contribution>);

impl Contributions {
    pub fn add(&mut self, kind: ContributionKind, what: impl Into<String>) {
        let what = what.into();
        if !self.0.iter().any(|known| known.kind == kind && known.what == what) {
            self.0.push(Contribution { kind, what });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn items(&self) -> &[Contribution] {
        &self.0
    }

    pub fn grouped(&self) -> Vec<(ContributionKind, Vec<&str>)> {
        ContributionKind::ALL
            .into_iter()
            .map(|kind| (kind, self.0.iter().filter(|item| item.kind == kind).map(|item| item.what.as_str()).collect()))
            .filter(|(_, items): &(_, Vec<&str>)| !items.is_empty())
            .collect()
    }

    pub fn listing(&self) -> String {
        let groups = self.grouped().into_iter().map(|(kind, items)| {
            let lines: Vec<String> = items.iter().map(|item| format!("    {item}")).collect();
            format!("  {}:\n{}", kind.title(), lines.join("\n"))
        });
        groups.collect::<Vec<_>>().join("\n")
    }

    pub fn listing_within(&self, lines: usize) -> String {
        let listing = self.listing();
        let all: Vec<&str> = listing.lines().collect();
        if all.len() <= lines {
            return listing;
        }
        format!("{}\n    … and {} more (/trust lists everything)", all[..lines].join("\n"), all.len() - lines)
    }

    pub fn counted(&self) -> String {
        let groups = self.grouped().into_iter().map(|(kind, items)| format!("{} ({})", kind.title(), items.len()));
        groups.collect::<Vec<_>>().join(", ")
    }

    pub fn skipped_notice(&self) -> Option<String> {
        (!self.is_empty()).then(|| format!("Skipped: {NOT_TRUSTED}.\n{}", self.listing()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustReport {
    pub root: PathBuf,
    pub trust: Trust,
    #[serde(default)]
    pub refusal: Option<Refusal>,
    #[serde(default)]
    pub brings: Contributions,
}

impl TrustReport {
    pub fn asks(&self) -> bool {
        self.trust == Trust::Untrusted && self.refusal.is_none() && !self.brings.is_empty()
    }

    pub fn skipped(&self) -> Option<&Contributions> {
        (self.trust == Trust::Untrusted && !self.brings.is_empty()).then_some(&self.brings)
    }

    pub fn state(&self) -> String {
        let folder = self.root.display();
        match (self.trust, self.refusal) {
            (Trust::Trusted, _) => format!("{folder} is trusted"),
            (Trust::Untrusted, Some(refusal)) => format!("{folder} is not trusted: {refusal}"),
            (Trust::Untrusted, None) if self.brings.is_empty() => {
                format!("{folder} is not trusted, and brings nothing that trust would enable")
            }
            (Trust::Untrusted, None) => {
                format!("{folder} is not trusted: {} skipped", plural(self.brings.len(), "project item"))
            }
        }
    }

    pub fn described(&self) -> String {
        let state = self.state();
        if self.brings.is_empty() {
            return format!("{state}.");
        }
        let verb = if self.trust.is_trusted() { "It brings" } else { "Trusting it would enable" };
        format!("{state}.\n{verb}:\n{}", self.brings.listing())
    }

    pub fn skipped_line(&self) -> Option<String> {
        let skipped = self.skipped()?;
        Some(format!(
            "this folder is not trusted, so {} skipped: {}. Pass --trust to load them for this run.",
            plural_were(skipped.len()),
            skipped.counted()
        ))
    }
}

fn plural(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

fn plural_were(count: usize) -> String {
    format!("{} {}", plural(count, "project item"), if count == 1 { "was" } else { "were" })
}
