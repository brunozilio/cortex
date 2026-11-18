use std::path::PathBuf;

use domain::brief_edits::{self, SkillEntry};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{HookEvent, Hooks};
use crate::brief::BriefError;
use crate::prompt::Instructions;
use crate::prompt::rules::Rule;
use crate::tools::Workspace;
use crate::tools::skill::FoundSkill;
use crate::tools::subagent::FoundAgent;

#[derive(Debug, Clone, Default)]
pub struct Briefed {
    pub skills: Vec<FoundSkill>,
    pub agents: Vec<FoundAgent>,
    pub rules: Vec<Rule>,
    pub instructions: Vec<Instructions>,
}

impl Hooks {
    pub fn edit_brief(&self, mut briefed: Briefed, workspace: &Workspace) -> Result<Briefed, BriefError> {
        for group in self.0.get(&HookEvent::Brief).into_iter().flatten() {
            let one = Hooks([(HookEvent::Brief, vec![group.clone()])].into());
            let shown = briefed.shown().map_err(BriefError::Hook)?;
            let decision = one.run(HookEvent::Brief, "", shown, workspace).map_err(BriefError::Hook)?;
            if let Some(reason) = decision.blocked {
                return Err(BriefError::StoppedByHook(reason));
            }
            if let Some(returned) = decision.brief {
                briefed = briefed.edited(&returned).map_err(BriefError::EditedBadly)?;
            }
        }
        Ok(briefed)
    }
}

impl Briefed {
    fn shown(&self) -> Result<Value, String> {
        Ok(json!({
            "skills": self.skills.iter().map(listed).collect::<Vec<_>>(),
            "agents": as_json(&self.agents)?,
            "rules": as_json(&self.rules)?,
            "instructions": as_json(&self.instructions)?,
        }))
    }

    fn edited(self, returned: &Value) -> Result<Self, String> {
        let skills = match returned.get("skills") {
            Some(skills) => applied(self.skills, skills)?,
            None => self.skills,
        };
        let agents: Vec<FoundAgent> = replaced(returned, "agents", self.agents)?;
        brief_edits::unique("agent", agents.iter().map(|agent| agent.name.as_str()))?;
        Ok(Self {
            skills,
            agents,
            rules: replaced(returned, "rules", self.rules)?,
            instructions: replaced(returned, "instructions", self.instructions)?,
        })
    }
}

fn as_json<T: Serialize>(items: &[T]) -> Result<Value, String> {
    serde_json::to_value(items).map_err(|error| error.to_string())
}

fn replaced<T: DeserializeOwned>(returned: &Value, key: &str, current: Vec<T>) -> Result<Vec<T>, String> {
    match returned.get(key) {
        Some(items) => serde_json::from_value(items.clone()).map_err(|error| format!("{key}: {error}")),
        None => Ok(current),
    }
}

fn listed(skill: &FoundSkill) -> Value {
    json!({
        "name": skill.name,
        "description": skill.description,
        "path": skill.file.display().to_string(),
        "body": skill.body,
        "disableModelInvocation": !skill.model_invocation,
    })
}

fn entry(skill: &FoundSkill) -> SkillEntry {
    SkillEntry {
        name: skill.name.clone(),
        description: skill.description.clone(),
        model_invocation: skill.model_invocation,
        body: skill.body.clone(),
    }
}

fn applied(skills: Vec<FoundSkill>, returned: &Value) -> Result<Vec<FoundSkill>, String> {
    let entries: Vec<SkillEntry> = skills.iter().map(entry).collect();
    let edited = brief_edits::edit(&entries, returned)?;
    Ok(edited
        .into_iter()
        .map(|edited| {
            let SkillEntry { name, description, model_invocation, body } = edited.skill;
            match edited.kept {
                Some(at) => FoundSkill { name, description, model_invocation, body, ..skills[at].clone() },
                None => FoundSkill { name, description, file: PathBuf::new(), model_invocation, trusted: false, body },
            }
        })
        .collect())
}
