use std::collections::HashSet;

use super::Standing;
use crate::permission_rules::{PermissionRules, RuleKind};

#[derive(Debug, Default, Clone)]
pub struct Remembered {
    answers: HashSet<String>,
    allowed: PermissionRules,
}

impl Remembered {
    pub fn allow_rule(&mut self, rule: &str) {
        let _ = self.allowed.add(RuleKind::Allow, rule);
    }

    pub fn rules(&self) -> &PermissionRules {
        &self.allowed
    }

    fn answer(tool: &str, standing: Standing, subject: &str) -> String {
        match standing {
            Standing::Edits => tool.to_owned(),
            Standing::Reads | Standing::Acts | Standing::ReadsOutside | Standing::EditsGuarded => {
                format!("{tool}({subject})")
            }
        }
    }

    pub fn remember(&mut self, tool: &str, standing: Standing, subject: &str) {
        self.answers.insert(Self::answer(tool, standing, subject));
    }

    pub fn allows(&self, tool: &str, standing: Standing, subject: &str) -> bool {
        self.answers.contains(&Self::answer(tool, standing, subject))
    }
}
