use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub model_invocation: bool,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    pub kept: Option<usize>,
    pub skill: SkillEntry,
}

pub fn edit(found: &[SkillEntry], returned: &Value) -> Result<Vec<Edited>, String> {
    let entries = returned.as_array().ok_or("skills must be a list")?;
    let mut edited: Vec<Edited> = Vec::new();
    for entry in entries {
        let name = entry["name"].as_str().map(str::trim).filter(|name| !name.is_empty());
        let name = name.ok_or("every skill the hook returns needs a name")?;
        if name.starts_with('/') || name.contains(char::is_whitespace) {
            return Err(format!("the skill name {name:?} has spaces or a leading slash"));
        }
        let kept = found.iter().position(|skill| skill.name == name);
        edited.push(Edited { kept, skill: entry_over(kept.map(|at| &found[at]), name, entry)? });
    }
    unique("skill", edited.iter().map(|edited| edited.skill.name.as_str()))?;
    Ok(edited)
}

pub fn unique<'a>(kind: &str, names: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let mut seen: Vec<&str> = Vec::new();
    for name in names {
        if seen.contains(&name) {
            return Err(format!("the hook listed the {kind} {name:?} twice"));
        }
        seen.push(name);
    }
    Ok(())
}

fn entry_over(original: Option<&SkillEntry>, name: &str, entry: &Value) -> Result<SkillEntry, String> {
    let text = |key: &str| entry[key].as_str().map(str::to_owned);
    let body = text("body").or_else(|| original.and_then(|skill| skill.body.clone()));
    let description = text("description").or_else(|| original.map(|skill| skill.description.clone()));
    let model_invocation = match entry["disableModelInvocation"].as_bool() {
        Some(disabled) => !disabled,
        None => original.is_none_or(|skill| skill.model_invocation),
    };
    if original.is_none() && body.is_none() {
        return Err(format!("the hook added the skill {name:?} without a body"));
    }
    let description = description.ok_or_else(|| format!("the hook added the skill {name:?} without a description"))?;
    Ok(SkillEntry { name: name.to_owned(), description, model_invocation, body })
}

#[cfg(test)]
#[path = "brief_edits_tests.rs"]
mod tests;
