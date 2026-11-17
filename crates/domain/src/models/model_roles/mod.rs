use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::approval::PermissionMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelRole {
    Plan,
    Explore,
    Small,
    Review,
    Advisor,
}

impl ModelRole {
    pub const ALL: [Self; 5] = [Self::Plan, Self::Explore, Self::Small, Self::Review, Self::Advisor];

    pub fn name(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Explore => "explore",
            Self::Small => "small",
            Self::Review => "review",
            Self::Advisor => "advisor",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Self::Plan => "plan mode and the Plan agent",
            Self::Explore => "the Explore agent that searches the code",
            Self::Small => "background chores: titles, effort picks, web reading",
            Self::Review => "the judge that checks finished work",
            Self::Advisor => "the advisor the agent consults",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim();
        Self::ALL.into_iter().find(|role| role.name().eq_ignore_ascii_case(wanted))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelRoles(BTreeMap<ModelRole, String>);

impl ModelRoles {
    pub fn get(&self, role: ModelRole) -> Option<&str> {
        self.0.get(&role).map(String::as_str)
    }

    pub fn set(&mut self, role: ModelRole, model: Option<String>) {
        match model.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()) {
            Some(model) => self.0.insert(role, model),
            None => self.0.remove(&role),
        };
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ModelRole, &str)> {
        self.0.iter().map(|(role, model)| (*role, model.as_str()))
    }

    pub fn for_turn<'a>(&'a self, session: &'a str, permission: PermissionMode) -> &'a str {
        match permission {
            PermissionMode::Plan => self.get(ModelRole::Plan).unwrap_or(session),
            _ => session,
        }
    }

    pub fn for_agent<'a>(&'a self, agent: &str, small: Option<&'a str>) -> Option<&'a str> {
        match agent {
            "Plan" => self.get(ModelRole::Plan),
            "Explore" => self.get(ModelRole::Explore).or(small),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
