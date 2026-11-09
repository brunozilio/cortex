//! Which tool calls run at once, which ask the user and which are refused,
//! for each permission mode. Pure.

use std::collections::HashSet;

use superfik_core::PermissionMode;

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
}

impl Remembered {
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

#[cfg(test)]
mod tests {
    use super::*;

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
