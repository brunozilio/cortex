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
}

fn invocation_enabled() -> bool {
    true
}
pub fn discover(request: &PromptRequest, home: Option<&Path>) -> Vec<FoundSkill> {
    let cwd = &request.cwd;
    let mut sources: Vec<(PathBuf, Option<String>)> = Vec::new();
    for directory in cwd.ancestors() {
        sources.push((directory.join(concat!(brand::dir!(), "/skills")), None));
    }
    if let Some(home) = home {
        sources.push((home.join(concat!(brand::dir!(), "/skills")), None));
    }
    for plugin in crate::plugins::found(request, home) {
        sources.push((plugin.root.join("skills"), Some(plugin.name)));
    }
    for dir in request.extensions.iter().flat_map(|extensions| &extensions.dirs) {
        sources.push((dir.join("skills"), None));
    }
    if let Some(builtin) = home.and_then(super::builtin_skills::dir) {
        sources.push((builtin, None));
    }
    let mut found: Vec<FoundSkill> = Vec::new();
    for (dir, namespace) in sources {
        for skill in scan(&dir, namespace.as_deref())
            .into_iter()
            .chain(scan_commands(&dir.with_file_name("commands"), namespace.as_deref()))
        {
            if !found.iter().any(|known| known.name == skill.name) {
                found.push(skill);
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
                path.is_file() && path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("skill.md"))
            })?;
            let text = std::fs::read_to_string(&file).ok()?;
            let (front, body) = parse_frontmatter(&text);
            let folder = entry.file_name().to_string_lossy().into_owned();
            let bare = front.get("name").unwrap_or(&folder).trim().to_owned();
            let description = front.get("description").map(str::to_owned).unwrap_or_else(|| {
                body.lines().find(|l| !l.trim().is_empty() && !l.starts_with('#')).unwrap_or("").to_owned()
            });
            let name = match namespace {
                Some(namespace) => format!("{namespace}:{bare}"),
                None => bare,
            };
            let model_invocation = front.get("disable-model-invocation").is_none_or(|v| v != "true");
            Some(FoundSkill { name, description: description.trim().to_owned(), file, model_invocation })
        })
        .collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

pub fn listing(skills: &[FoundSkill]) -> String {
    skills
        .iter()
        .filter(|skill| skill.model_invocation)
        .map(|skill| format!("- {}: {}", skill.name, skill.description))
        .collect::<Vec<_>>()
        .join("\n")
}

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
            description: "Loads a skill's instructions by its exact name from the list of available skills. When a \
                          skill covers the task, call this first and follow what it returns."
                .to_owned(),
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

    fn run(&self, input: &Value, _workspace: &Workspace) -> Outcome {
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
        load(skill, input["args"].as_str().unwrap_or_default())
    }
}

fn load(skill: &FoundSkill, args: &str) -> Outcome {
    let name = &skill.name;
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
pub fn expand_initial(initial: &str, skills: &[FoundSkill]) -> Result<String, String> {
    let initial = initial.trim();
    let Some(command) = initial.strip_prefix('/') else { return Ok(initial.into()) };
    expand_command(initial, skills).unwrap_or_else(|| {
        let name = command.split_whitespace().next().unwrap_or_default();
        Err(format!("Initial command /{name} is not an available skill or custom command"))
    })
}

pub fn expand_command(prompt: &str, skills: &[FoundSkill]) -> Option<Result<String, String>> {
    let command = prompt.trim().strip_prefix('/')?;
    let (name, args) = command.split_once(char::is_whitespace).unwrap_or((command, ""));
    let skill = skills.iter().find(|skill| skill.name == name)?;
    let loaded = load(skill, args.trim());
    Some(if loaded.is_error { Err(loaded.content) } else { Ok(loaded.content) })
}
fn scan_commands(dir: &Path, namespace: Option<&str>) -> Vec<FoundSkill> {
    let mut found = Vec::new();
    for entry in ignore::WalkBuilder::new(dir).hidden(false).max_depth(Some(8)).build().filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) || entry.path().extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        let (front, body) = parse_frontmatter(&text);
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
        });
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

#[cfg(test)]
mod tests {
    use domain::{Extensions, ModelOption};

    use super::*;

    fn skill(dir: &Path, name: &str, description: &str) {
        std::fs::create_dir_all(dir.join(name)).expect("dir");
        let text = format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\nDo the {name} thing.\n");
        std::fs::write(dir.join(name).join("SKILL.md"), text).expect("write");
    }

    #[test]
    fn finds_the_project_s_the_user_s_and_the_host_s_skills() {
        let repo = tempfile::tempdir().expect("repo");
        let home = tempfile::tempdir().expect("home");
        let plugin = tempfile::tempdir().expect("plugin");
        skill(&repo.path().join(concat!(brand::dir!(), "/skills")), "release", "Cuts a release.");
        skill(&home.path().join(concat!(brand::dir!(), "/skills")), "release", "The user's copy.");
        skill(&home.path().join(concat!(brand::dir!(), "/skills")), "morning", "The morning briefing.");
        skill(&plugin.path().join("skills"), "dataviz", "Charts.");
        let model = ModelOption::new("m", "M");
        let mut request = PromptRequest::plain(repo.path().to_path_buf(), "hi", model);
        request.extensions = Some(Extensions { dirs: vec![plugin.path().to_path_buf()], ..Extensions::default() });
        let found = discover(&request, Some(home.path()));
        let names: Vec<(&str, &str)> = found.iter().map(|s| (s.name.as_str(), s.description.as_str())).collect();
        assert_eq!(
            &names[..3],
            [("release", "Cuts a release."), ("morning", "The morning briefing."), ("dataviz", "Charts.")],
            "the project's, the user's and the host's first"
        );
        assert!(
            names[3..].iter().any(|(name, _)| *name == "code-review"),
            "then {brand_name}'s own",
            brand_name = brand::NAME
        );
        assert_eq!(listing(&found[..1]), "- release: Cuts a release.");

        let tool = SkillTool { skills: found };
        let loaded = tool.run(&json!({ "skill": "/dataviz", "args": "bar chart" }), &Workspace::new("/"));
        assert!(!loaded.is_error);
        assert!(loaded.content.starts_with("Skill dataviz (base directory for its files: "));
        assert!(loaded.content.contains("Do the dataviz thing.") && loaded.content.ends_with("Arguments: bar chart"));
        assert!(!loaded.content.contains("description:"), "the frontmatter stays behind");
        assert!(tool.run(&json!({ "skill": "missing" }), &Workspace::new("/")).is_error);
    }
}
