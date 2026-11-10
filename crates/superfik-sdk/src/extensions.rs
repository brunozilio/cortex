//! Agents, skills and commands written in code. They are laid out as a
//! folder in the coding agents' layout (`agents/<name>.md`,
//! `skills/<name>/SKILL.md`, `commands/<name>.md`), offered to every run
//! under their own names.

use std::path::Path;

/// A sub-agent the main agent can hand work to (or a session can run as).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    name: String,
    description: String,
    instructions: String,
    tools: Vec<String>,
    model: Option<String>,
}

impl Agent {
    /// `description` says when to use it: the main agent reads it to choose.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            instructions: String::new(),
            tools: Vec::new(),
            model: None,
        }
    }

    /// Its system prompt.
    pub fn instructions(mut self, text: impl Into<String>) -> Self {
        self.instructions = text.into();
        self
    }

    /// The only tools it may use (`Read`, `Grep`, `mcp__math__*`, …); all
    /// when none are named.
    pub fn tools<S: Into<String>>(mut self, tools: impl IntoIterator<Item = S>) -> Self {
        self.tools = tools.into_iter().map(Into::into).collect();
        self
    }

    /// A model of its own (`sonnet`, `opus`, `haiku`, `inherit` or an id).
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    fn markdown(&self) -> String {
        let mut front = vec![format!("name: {}", self.name), format!("description: {}", one_line(&self.description))];
        if !self.tools.is_empty() {
            front.push(format!("tools: {}", self.tools.join(", ")));
        }
        if let Some(model) = &self.model {
            front.push(format!("model: {model}"));
        }
        document(&front, &self.instructions)
    }
}

/// Instructions the agent loads by name when they fit the task (and the
/// user can run as `/<name>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    name: String,
    description: String,
    body: String,
}

impl Skill {
    pub fn new(name: impl Into<String>, description: impl Into<String>, body: impl Into<String>) -> Self {
        Self { name: name.into(), description: description.into(), body: body.into() }
    }

    fn markdown(&self) -> String {
        document(&[format!("name: {}", self.name), format!("description: {}", one_line(&self.description))], &self.body)
    }
}

/// A prompt the user runs as `/<name> <arguments>`; `$ARGUMENTS` in its
/// body is replaced by what follows the name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    name: String,
    description: String,
    body: String,
}

impl Command {
    pub fn new(name: impl Into<String>, description: impl Into<String>, body: impl Into<String>) -> Self {
        Self { name: name.into(), description: description.into(), body: body.into() }
    }

    fn markdown(&self) -> String {
        document(&[format!("description: {}", one_line(&self.description))], &self.body)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Definitions {
    pub agents: Vec<Agent>,
    pub skills: Vec<Skill>,
    pub commands: Vec<Command>,
}

impl Definitions {
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty() && self.skills.is_empty() && self.commands.is_empty()
    }

    /// Replaces `root` with this folder.
    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        if root.exists() {
            std::fs::remove_dir_all(root)?;
        }
        for agent in &self.agents {
            write(&root.join("agents").join(file_name(&agent.name, "md")?), &agent.markdown())?;
        }
        for skill in &self.skills {
            write(&root.join("skills").join(file_name(&skill.name, "")?).join("SKILL.md"), &skill.markdown())?;
        }
        for command in &self.commands {
            write(&root.join("commands").join(file_name(&command.name, "md")?), &command.markdown())?;
        }
        Ok(())
    }
}

fn file_name(name: &str, extension: &str) -> std::io::Result<String> {
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !name.starts_with('.');
    if !valid {
        let why = format!("`{name}` is not a valid name: use letters, digits, `-`, `_` or `.`");
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, why));
    }
    Ok(if extension.is_empty() { name.to_owned() } else { format!("{name}.{extension}") })
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn document(front: &[String], body: &str) -> String {
    format!("---\n{}\n---\n{}\n", front.join("\n"), body.trim())
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_laid_out_like_a_coding_agent_s_folder() {
        let dir = tempfile::tempdir().expect("dir");
        let root = dir.path().join("sdk");
        let definitions = Definitions {
            agents: vec![
                Agent::new("reviewer", "Reviews diffs.\nUse it after edits.")
                    .instructions("Review carefully.")
                    .tools(["Read", "Grep"])
                    .model("sonnet"),
            ],
            skills: vec![Skill::new("release", "Cuts a release.", "1. Tag.")],
            commands: vec![Command::new("audit", "Audits a path.", "Audit $ARGUMENTS")],
        };
        std::fs::create_dir_all(root.join("stale")).expect("stale");
        definitions.write(&root).expect("write");
        let read = |path: &str| std::fs::read_to_string(root.join(path)).expect(path);
        assert_eq!(
            read("agents/reviewer.md"),
            "---\nname: reviewer\ndescription: Reviews diffs. Use it after edits.\ntools: Read, Grep\nmodel: sonnet\n---\nReview carefully.\n"
        );
        assert!(read("skills/release/SKILL.md").starts_with("---\nname: release\ndescription: Cuts a release.\n---"));
        assert_eq!(read("commands/audit.md"), "---\ndescription: Audits a path.\n---\nAudit $ARGUMENTS\n");
        assert!(!root.join("stale").exists(), "the folder is rewritten");
    }

    #[test]
    fn a_name_cannot_leave_its_folder() {
        let dir = tempfile::tempdir().expect("dir");
        let definitions = Definitions { skills: vec![Skill::new("../evil", "x", "y")], ..Definitions::default() };
        assert!(definitions.write(&dir.path().join("sdk")).is_err());
    }
}
