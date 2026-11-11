//! Which tool calls run at once, which ask the user and which are refused,
//! for each permission mode. Pure.

use std::collections::HashSet;

use superfik_core::PermissionMode;
use superfik_core::permission_rules::{PermissionRules, RuleKind};

use crate::tools::Access;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Ask,
    Deny(String),
}

#[derive(Debug, Default, Clone)]
pub struct Remembered {
    rules: HashSet<String>,
    /// Allow rules the user accepted during the run ("don't ask again").
    allowed: PermissionRules,
}

impl Remembered {
    pub fn allow_rule(&mut self, rule: &str) {
        let _ = self.allowed.add(RuleKind::Allow, rule);
    }

    pub fn rules(&self) -> &PermissionRules {
        &self.allowed
    }

    fn rule(tool: &str, access: Access, subject: &str) -> String {
        match access {
            Access::Edits => tool.to_owned(),
            Access::Reads | Access::Executes | Access::ReadsOutside => format!("{tool}({subject})"),
        }
    }

    pub fn remember(&mut self, tool: &str, access: Access, subject: &str) {
        self.rules.insert(Self::rule(tool, access, subject));
    }

    pub fn allows(&self, tool: &str, access: Access, subject: &str) -> bool {
        self.rules.contains(&Self::rule(tool, access, subject))
    }
}

pub fn verdict(mode: PermissionMode, access: Access, tool: &str, subject: &str, remembered: &Remembered) -> Verdict {
    match (mode, access) {
        (_, Access::Reads) | (PermissionMode::Bypass, _) => Verdict::Allow,
        (_, Access::ReadsOutside) if remembered.allows(tool, access, subject) => Verdict::Allow,
        (_, Access::ReadsOutside) => Verdict::Ask,
        (PermissionMode::Plan, _) => Verdict::Deny(format!(
            "{tool} is not available while planning: only read and search now, then present the plan."
        )),
        (PermissionMode::AcceptEdits, Access::Edits) => Verdict::Allow,
        _ if remembered.allows(tool, access, subject) => Verdict::Allow,
        _ => Verdict::Ask,
    }
}

/// The mode's `verdict` under the settings' permission rules: a deny rule
/// refuses even when bypassing; otherwise bypassing allows and planning
/// keeps refusing what is not a read, and past those an ask rule asks and an
/// allow rule allows.
pub fn ruled(mode: PermissionMode, access: Access, rule: Option<(RuleKind, String)>, by_mode: Verdict) -> Verdict {
    let planning = mode == PermissionMode::Plan && matches!(access, Access::Edits | Access::Executes);
    match rule {
        Some((RuleKind::Deny, rule)) => Verdict::Deny(format!(
            "The permission rule {rule} in settings refuses this. Do not retry it; take another approach or ask the user."
        )),
        _ if mode == PermissionMode::Bypass || planning => by_mode,
        Some((RuleKind::Ask, _)) => Verdict::Ask,
        Some((RuleKind::Allow, _)) => Verdict::Allow,
        None => by_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_refuse_ask_or_allow_over_the_mode_but_never_lift_planning() {
        let deny = || Some((RuleKind::Deny, "Bash(rm:*)".to_owned()));
        let ask = || Some((RuleKind::Ask, "Edit(docs/**)".to_owned()));
        let allow = || Some((RuleKind::Allow, String::new()));
        let bypass = PermissionMode::Bypass;
        assert!(
            matches!(ruled(bypass, Access::Executes, deny(), Verdict::Allow), Verdict::Deny(r) if r.contains("Bash(rm:*)"))
        );
        assert_eq!(ruled(bypass, Access::Edits, ask(), Verdict::Allow), Verdict::Allow);
        assert_eq!(ruled(PermissionMode::AcceptEdits, Access::Edits, ask(), Verdict::Allow), Verdict::Ask);
        assert_eq!(ruled(PermissionMode::Ask, Access::Reads, ask(), Verdict::Allow), Verdict::Ask);
        assert_eq!(ruled(PermissionMode::Ask, Access::Executes, allow(), Verdict::Ask), Verdict::Allow);
        let planning = Verdict::Deny("planning".into());
        assert_eq!(ruled(PermissionMode::Plan, Access::Executes, allow(), planning.clone()), planning);
        assert_eq!(ruled(PermissionMode::Plan, Access::ReadsOutside, allow(), Verdict::Ask), Verdict::Allow);
        assert_eq!(ruled(PermissionMode::Ask, Access::Executes, None, Verdict::Ask), Verdict::Ask);
    }

    #[test]
    fn each_mode_decides_by_access() {
        let none = Remembered::default();
        let cases = [
            (PermissionMode::Ask, Access::Reads, Verdict::Allow),
            (PermissionMode::Ask, Access::Edits, Verdict::Ask),
            (PermissionMode::Ask, Access::Executes, Verdict::Ask),
            (PermissionMode::AcceptEdits, Access::Edits, Verdict::Allow),
            (PermissionMode::AcceptEdits, Access::Executes, Verdict::Ask),
            (PermissionMode::Bypass, Access::Executes, Verdict::Allow),
            (PermissionMode::Plan, Access::Reads, Verdict::Allow),
        ];
        for (mode, access, expected) in cases {
            assert_eq!(verdict(mode, access, "T", "s", &none), expected, "{mode:?} {access:?}");
        }
        assert!(matches!(verdict(PermissionMode::Plan, Access::Edits, "Edit", "a", &none), Verdict::Deny(_)));
    }

    #[test]
    fn reading_outside_the_project_is_asked_for_unless_bypassing() {
        let none = Remembered::default();
        let outside = Access::ReadsOutside;
        for mode in [PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Plan] {
            assert_eq!(verdict(mode, outside, "Read", "/home/a/.ssh/id", &none), Verdict::Ask, "{mode:?}");
        }
        assert_eq!(verdict(PermissionMode::Bypass, outside, "Read", "/home/a/.ssh/id", &none), Verdict::Allow);
        let mut remembered = Remembered::default();
        remembered.remember("Read", outside, "/etc/hosts");
        assert_eq!(verdict(PermissionMode::Plan, outside, "Read", "/etc/hosts", &remembered), Verdict::Allow);
        assert_eq!(verdict(PermissionMode::Ask, outside, "Read", "/etc/passwd", &remembered), Verdict::Ask, "by path");
    }

    #[test]
    fn always_covers_a_tool_for_edits_and_a_command_for_the_rest() {
        let mut remembered = Remembered::default();
        remembered.remember("Edit", Access::Edits, "a.rs");
        remembered.remember("Bash", Access::Executes, "cargo test");
        let ask = PermissionMode::Ask;
        assert_eq!(verdict(ask, Access::Edits, "Edit", "b.rs", &remembered), Verdict::Allow);
        assert_eq!(verdict(ask, Access::Executes, "Bash", "cargo test", &remembered), Verdict::Allow);
        assert_eq!(verdict(ask, Access::Executes, "Bash", "rm -rf x", &remembered), Verdict::Ask);
        assert!(matches!(verdict(PermissionMode::Plan, Access::Edits, "Edit", "b", &remembered), Verdict::Deny(_)));
    }
}
