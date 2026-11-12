use std::path::{Path, PathBuf};

use crate::frontmatter::parse as parse_frontmatter;
use domain::PromptRequest;
use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, required};
use crate::model::ToolSpec;

pub const SKILL_TOOL: &str = "Skill";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FoundSkill {
    pub name: String,
    pub description: String,
    pub file: PathBuf,
    #[serde(default = "invocation_enabled")]
    pub model_invocation: bool,
    #[serde(default = "invocation_enabled")]
    pub trusted: bool,
}

impl FoundSkill {
    pub fn project_script(&self) -> bool {
        !self.trusted && super::skill_script::is_script(&self.file)
    }
}

fn invocation_enabled() -> bool {
    true
}
pub fn discover(request: &PromptRequest, home: Option<&Path>) -> Vec<FoundSkill> {
    let cwd = &request.cwd;
    let mut sources: Vec<(PathBuf, Option<String>, bool)> = Vec::new();
    for directory in cwd.ancestors() {
        let owners = home.is_some_and(|home| home == directory);
        sources.push((directory.join(concat!(brand::dir!(), "/skills")), None, owners));
    }
    if let Some(home) = home {
        sources.push((home.join(concat!(brand::dir!(), "/skills")), None, true));
    }
    for plugin in crate::plugins::found(request, home) {
        sources.push((plugin.root.join("skills"), Some(plugin.name), true));
    }
    for dir in request.extensions.iter().flat_map(|extensions| &extensions.dirs) {
        sources.push((dir.join("skills"), None, true));
    }
    if let Some(builtin) = home.and_then(super::builtin_skills::dir) {
        sources.push((builtin, None, true));
    }
    let mut found: Vec<FoundSkill> = Vec::new();
    for (dir, namespace, trusted) in sources {
        for skill in scan(&dir, namespace.as_deref())
            .into_iter()
            .chain(scan_commands(&dir.with_file_name("commands"), namespace.as_deref()))
        {
            let skill = FoundSkill { trusted, ..skill };
            match found.iter().position(|known| known.name == skill.name) {
                None => found.push(skill),
                Some(at) if skill.trusted && found[at].project_script() => found[at] = skill,
                Some(_) => {}
            }
        }
    }
    found
}
fn scan(dir: &Path, namespace: Option<&str>) -> Vec<FoundSkill> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut skills: Vec<FoundSkill> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let file = std::fs::read_dir(entry.path()).ok()?.flatten().map(|e| e.path()).find(|path| {
                path.is_file()
                    && path.file_name().is_some_and(|name| {
                        name.eq_ignore_ascii_case("skill.md") || name.eq_ignore_ascii_case("skill.js")
                    })
            })?;
            let folder = entry.file_name().to_string_lossy().into_owned();
            if super::skill_script::is_script(&file) {
                let described = super::skill_script::describe(&file)?;
                let bare = described.name.unwrap_or(folder);
                let name = namespace.map_or_else(|| bare.clone(), |namespace| format!("{namespace}:{bare}"));
                return Some(FoundSkill {
                    name,
                    description: described.description,
                    file,
                    model_invocation: described.model_invocation,
                    trusted: true,
                });
            }
            let text = std::fs::read_to_string(&file).ok()?;
            let (front, body) = parse_frontmatter(&text);
            let bare = front.get("name").unwrap_or(&folder).trim().to_owned();
            let description = front.get("description").map(str::to_owned).unwrap_or_else(|| {
                body.lines().find(|l| !l.trim().is_empty() && !l.starts_with('#')).unwrap_or("").to_owned()
            });
            let name = match namespace {
                Some(namespace) => format!("{namespace}:{bare}"),
                None => bare,
            };
            let model_invocation = front.get("disable-model-invocation").is_none_or(|v| v != "true");
            Some(FoundSkill { name, description: description.trim().to_owned(), file, model_invocation, trusted: true })
        })
        .collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

pub use super::skill_listing::listing;

#[derive(Debug, Clone, Default)]
pub struct SkillTool {
    pub skills: Vec<FoundSkill>,
}

impl Tool for SkillTool {
    fn skills(&self) -> &[FoundSkill] {
        &self.skills
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: SKILL_TOOL.to_owned(),
            description: crate::tools::describe("Skill", &serde_json::Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "skill": { "type": "string", "description": "The skill's name, without a leading slash." },
                    "args": { "type": "string", "description": "Optional arguments for the skill." },
                },
                "required": ["skill"],
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        input["skill"].as_str().unwrap_or_default().trim_start_matches('/').to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let name = match required(input, "skill") {
            Ok(name) => name.trim_start_matches('/'),
            Err(outcome) => return outcome,
        };
        let Some(skill) = self.skills.iter().find(|skill| skill.name == name) else {
            return Outcome::error(format!("There is no skill named {name}. Use one of the available skills."));
        };
        if !skill.model_invocation {
            return Outcome::error(format!("Skill {name} requires explicit user invocation."));
        }
        load(skill, input["args"].as_str().unwrap_or_default(), &workspace.cwd, workspace.scripts)
    }
}

fn load(skill: &FoundSkill, args: &str, cwd: &Path, access: ScriptAccess) -> Outcome {
    let name = &skill.name;
    if super::skill_script::is_script(&skill.file) {
        return match super::skill_script::render(&skill.file, args, cwd, access) {
            Ok(text) => Outcome::ok(text),
            Err(error) => Outcome::error(format!("The skill {name} failed: {error}")),
        };
    }
    let Ok(text) = std::fs::read_to_string(&skill.file) else {
        return Outcome::error(format!("The skill {name} could not be read."));
    };
    let (_, body) = parse_frontmatter(&text);
    let base = skill.file.parent().map(|dir| dir.display().to_string()).unwrap_or_default();
    let mut out =
        format!("Skill {name} (base directory for its files: {base})\n\n{}", body.trim().replace("$ARGUMENTS", args));
    if !args.trim().is_empty() {
        out.push_str(&format!("\n\nArguments: {args}"));
    }
    Outcome::ok(out)
}
use super::script_tool::ScriptAccess;

pub fn expand_initial(
    initial: &str,
    skills: &[FoundSkill],
    cwd: &Path,
    access: ScriptAccess,
) -> Result<String, String> {
    let initial = initial.trim();
    let Some(command) = initial.strip_prefix('/') else { return Ok(initial.into()) };
    let unattended = |skill: &FoundSkill| if skill.project_script() { ScriptAccess::none() } else { access };
    expand(initial, skills, cwd, unattended).unwrap_or_else(|| {
        let name = command.split_whitespace().next().unwrap_or_default();
        Err(format!("Initial command /{name} is not an available skill or custom command"))
    })
}

pub fn expand_command(
    prompt: &str,
    skills: &[FoundSkill],
    cwd: &Path,
    access: ScriptAccess,
) -> Option<Result<String, String>> {
    expand(prompt, skills, cwd, |_| access)
}

fn expand(
    prompt: &str,
    skills: &[FoundSkill],
    cwd: &Path,
    access: impl Fn(&FoundSkill) -> ScriptAccess,
) -> Option<Result<String, String>> {
    let command = prompt.trim().strip_prefix('/')?;
    let (name, args) = command.split_once(char::is_whitespace).unwrap_or((command, ""));
    let skill = skills.iter().find(|skill| skill.name == name)?;
    let loaded = load(skill, args.trim(), cwd, access(skill));
    Some(if loaded.is_error { Err(loaded.content) } else { Ok(loaded.content) })
}
fn scan_commands(dir: &Path, namespace: Option<&str>) -> Vec<FoundSkill> {
    let mut found = Vec::new();
    for entry in ignore::WalkBuilder::new(dir).hidden(false).max_depth(Some(8)).build().filter_map(Result::ok) {
        let ext = entry.path().extension().and_then(|ext| ext.to_str()).unwrap_or_default().to_owned();
        if !entry.file_type().is_some_and(|t| t.is_file()) || !matches!(ext.as_str(), "md" | "js") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        let (front, body) = if ext == "js" {
            let Some(described) = super::skill_script::describe(entry.path()) else { continue };
            let mut pairs = vec![("description".to_owned(), described.description)];
            if !described.model_invocation {
                pairs.push(("disable-model-invocation".to_owned(), "true".to_owned()));
            }
            (crate::frontmatter::Frontmatter::from_pairs(pairs), String::new())
        } else {
            let (front, body) = parse_frontmatter(&text);
            (front, body.to_owned())
        };
        let Ok(relative) = entry.path().strip_prefix(dir) else { continue };
        let bare = relative
            .with_extension("")
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(":");
        let name = namespace.map_or_else(|| bare.clone(), |prefix| format!("{prefix}:{bare}"));
        let description = front
            .get("description")
            .map(str::to_owned)
            .unwrap_or_else(|| body.lines().next().unwrap_or_default().into());
        found.push(FoundSkill {
            name,
            description,
            file: entry.path().into(),
            model_invocation: front.get("disable-model-invocation").is_none_or(|value| value != "true"),
            trusted: true,
        });
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
