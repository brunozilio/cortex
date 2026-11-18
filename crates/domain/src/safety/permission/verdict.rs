use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    OutsideTheProject,
    GitFolder,
    OwnConfiguration,
}

impl Guard {
    pub fn why(self) -> &'static str {
        match self {
            Self::OutsideTheProject => "This writes outside the project and the folders you added:",
            Self::GitFolder => {
                "This writes inside the repository's .git folder, where hooks and configuration can run commands:"
            }
            Self::OwnConfiguration => {
                concat!(
                    "This changes ",
                    brand::name!(),
                    "'s own configuration, which can grant permissions and run code:"
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    Mode,
    Rule(String),
    Hook,
    Outside(PathBuf),
    Guarded { path: PathBuf, guard: Guard },
    Risky(String),
    Hidden(String),
    Address(String),
    LeavesTheSandbox,
    Package(String),
    Install { what: String, rule: String },
}

impl Reason {
    pub fn every_time(&self) -> bool {
        matches!(
            self,
            Self::Risky(_)
                | Self::Hidden(_)
                | Self::Address(_)
                | Self::LeavesTheSandbox
                | Self::Package(_)
                | Self::Install { .. }
        )
    }

    pub fn rememberable(&self) -> bool {
        matches!(self, Self::Mode | Self::Outside(_) | Self::Guarded { .. } | Self::Install { .. })
    }

    pub fn heading(&self) -> Option<String> {
        match self {
            Self::Risky(why) | Self::Address(why) => Some(why.clone()),
            Self::Hidden(rule) => Some(format!("{} Run it anyway?", hidden_heading(rule))),
            Self::LeavesTheSandbox => Some("Run OUTSIDE the sandbox (it may write anywhere):".to_owned()),
            Self::Package(concern) => Some(format!("Check the package first: {concern}")),
            Self::Install { .. } => Some("Install software on this computer first:".to_owned()),
            Self::Mode | Self::Rule(_) | Self::Hook | Self::Outside(_) | Self::Guarded { .. } => None,
        }
    }

    pub fn unattended(&self, tool: &str) -> String {
        match self {
            Self::Risky(why) | Self::Address(why) => format!("Not run: {why} The user cannot be asked here."),
            Self::Hidden(rule) => format!(
                "Not run: {} The user cannot be asked here; spell the command out so the rules can see it.",
                hidden_heading(rule)
            ),
            Self::LeavesTheSandbox => {
                "Not run: leaving the sandbox needs the user, who cannot be asked here.".to_owned()
            }
            Self::Package(concern) => format!("Not run: {concern}. Ask the user before installing it."),
            Self::Install { what, rule } => format!(
                "Not installed: {what} needs the user's consent, and the user cannot be asked here. To allow it \
                 without a question, the user adds the permission rule {rule} to permissions.allow in ~/{}/settings.json.",
                brand::DIR
            ),
            Self::Mode | Self::Rule(_) | Self::Hook | Self::Outside(_) | Self::Guarded { .. } => {
                format!("{tool} requires permission; dontAsk denies unapproved actions.")
            }
        }
    }
}

fn hidden_heading(rule: &str) -> String {
    format!(
        "This command names a program the permission rules cannot see (a shell's code, a variable, a substitution \
         or a wildcard), and the rule {rule} refuses what it might run."
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Ask { reason: Reason, rememberable: bool, rule: Option<String> },
    Deny(String),
}

impl Verdict {
    pub fn asks(&self) -> bool {
        matches!(self, Self::Ask { .. })
    }

    pub fn denies(&self) -> bool {
        matches!(self, Self::Deny(_))
    }

    pub fn asked_by_a_rule(&self) -> bool {
        matches!(self, Self::Ask { reason: Reason::Rule(_), .. })
    }

    pub fn asked_every_time(&self) -> bool {
        matches!(self, Self::Ask { reason, .. } if reason.every_time())
    }

    pub fn without_the_user(self, tool: &str) -> Self {
        match self {
            Self::Ask { reason, .. } => Self::Deny(reason.unattended(tool)),
            settled => settled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Reads,
    ReadsOutside,
    Edits,
    EditsGuarded,
    Acts,
}

impl Standing {
    pub fn refused_while_planning(self) -> bool {
        matches!(self, Self::Edits | Self::EditsGuarded | Self::Acts)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judgement {
    pub verdict: Verdict,
    pub standing: Standing,
    pub subject: String,
    pub guarded: Option<(PathBuf, Guard)>,
    pub rule: Option<String>,
}

impl Judgement {
    pub fn asking(&self, reason: Reason) -> Verdict {
        Verdict::Ask { rememberable: reason.rememberable(), reason, rule: self.rule.clone() }
    }
}
