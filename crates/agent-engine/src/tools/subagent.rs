use std::path::{Path, PathBuf};

use super::subagent_config::AgentConfig;
use domain::PromptRequest;
use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub const AGENT_TOOL: &str = "Agent";

pub const GENERAL: &str = "general-purpose";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FoundAgent {
    pub name: String,
    pub description: String,
    pub instructions: String,
    #[serde(default)]
    pub config: AgentConfig,
}

pub fn discover(request: &PromptRequest, home: Option<&Path>) -> Vec<FoundAgent> {
    let cwd = &request.cwd;
    let mut sources: Vec<(PathBuf, Option<String>)> = Vec::new();
    for directory in cwd.ancestors() {
        sources.push((directory.join(concat!(brand::dir!(), "/agents")), None));
    }
    if let Some(home) = home {
        sources.push((home.join(concat!(brand::dir!(), "/agents")), None));
    }
    for plugin in crate::plugins::found(request, home) {
        sources.push((plugin.root.join("agents"), Some(plugin.name)));
    }
    for dir in request.extensions.iter().flat_map(|extensions| &extensions.dirs) {
        sources.push((dir.join("agents"), None));
    }
    let mut found: Vec<FoundAgent> = Vec::new();
    let scanned = sources.into_iter().flat_map(|(dir, namespace)| scan(&dir, namespace.as_deref()));
    for agent in scanned.chain(super::builtin_agents::all()) {
        if !found.iter().any(|known| known.name == agent.name) {
            found.push(agent);
        }
    }
    found
}

fn scan(dir: &Path, namespace: Option<&str>) -> Vec<FoundAgent> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut agents: Vec<FoundAgent> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let (front, body, config) = match crate::prompt::markdown::frontmatter(&text) {
                Ok((mut front, body)) => {
                    if namespace.is_some()
                        && let Some(map) = front.as_object_mut()
                    {
                        for key in ["hooks", "mcpServers", "permissionMode", "initialPrompt"] {
                            map.remove(key);
                        }
                    }
                    let config = AgentConfig::parse(&front)
                        .unwrap_or_else(|error| AgentConfig { error: Some(error), ..Default::default() });
                    (front, body, config)
                }
                Err(error) => (Value::Null, "", AgentConfig { error: Some(error), ..Default::default() }),
            };
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let bare = front["name"].as_str().unwrap_or(&stem).trim().to_owned();
            let name = match namespace {
                Some(namespace) => format!("{namespace}:{bare}"),
                None => bare,
            };
            let description = front["description"].as_str().unwrap_or_default().trim().to_owned();
            Some(FoundAgent { name, description, instructions: body.trim().to_owned(), config })
        })
        .collect();
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

#[derive(Debug, Clone, Default)]
pub struct AgentTool {
    pub agents: Vec<FoundAgent>,
}

impl AgentTool {
    pub fn background(&self, input: &Value) -> bool {
        input["run_in_background"]
            .as_bool()
            .unwrap_or_else(|| self.chosen(input).ok().flatten().is_some_and(|agent| agent.config.background))
    }
    pub fn chosen(&self, input: &Value) -> Result<Option<&FoundAgent>, String> {
        let chosen = match input["subagent_type"].as_str().filter(|kind| !kind.is_empty() && *kind != GENERAL) {
            None => Ok(None),
            Some(kind) => self.agents.iter().find(|agent| agent.name == kind).map(Some).ok_or_else(|| {
                format!("There is no agent named {kind}. Leave subagent_type out for a general-purpose agent.")
            }),
        }?;
        if let Some(agent) = chosen
            && let Some(error) = &agent.config.error
        {
            return Err(format!("Agent {}: {error}", agent.name));
        }
        Ok(chosen)
    }
}

impl Tool for AgentTool {
    fn spec(&self) -> ToolSpec {
        let agents: Vec<Value> =
            self.agents.iter().map(|agent| json!({ "name": agent.name, "description": agent.description })).collect();
        let description = crate::tools::describe("Agent", &json!({ "agents": agents }));
        ToolSpec {
            name: AGENT_TOOL.to_owned(),
            description,
            schema: json!({
                "type": "object",
                "properties": {
                    "description": { "type": "string", "description": "A short description of the task, 3 to 5 words." },
                    "prompt": { "type": "string", "description": "The task for the agent to carry out." },
                    "max_turns":{"type":"integer","minimum":1,"maximum":200},
                    "run_in_background": { "type": "boolean", "description": "Run concurrently and return a task_id. Collect its report with TaskOutput." },
                    "subagent_type": { "type": "string", "description": "The kind of agent; general-purpose when left out." },
                    "model": { "type": "string", "description": "A model for this agent: an id or short name your own provider serves. One it does not serve is ignored and the agent's own or yours is used, as when left out." },
                    "isolation": { "enum": ["worktree"], "description": "worktree: work in a git worktree of its own, kept when it changed something." },
                },
                "required": ["description", "prompt"],
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        input["description"].as_str().unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("Sub-agents can only be started by the agent loop.")
    }
}

pub fn system(parent: &str, agent: Option<&FoundAgent>) -> String {
    let agent = agent.map(|agent| serde_json::json!({ "name": agent.name, "instructions": agent.instructions }));
    let data = serde_json::json!({ "parent": parent, "agent": agent, "general": GENERAL });
    crate::prompt::render("prompts/sub-agent.js", &data)
}

#[cfg(test)]
mod tests {
    use domain::ModelOption;

    use super::*;

    #[test]
    fn finds_custom_agents_and_picks_the_one_asked_for() {
        let repo = tempfile::tempdir().expect("repo");
        let dir = repo.path().join(concat!(brand::dir!(), "/agents"));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Reviews diffs.\n---\n\nBe strict.\n",
        )
        .expect("write");
        let model = ModelOption::new("m", "M");
        let request = PromptRequest::plain(repo.path().to_path_buf(), "hi", model);
        let tool = AgentTool { agents: discover(&request, None) };
        let description = tool.spec().description;
        assert!(description.contains("- reviewer: Reviews diffs.\n- Explore: "), "project agents first: {description}");
        assert!(description.contains("- fork: "));
        let reviewer = tool.chosen(&json!({ "subagent_type": "reviewer" })).expect("known").expect("custom");
        assert_eq!(reviewer.instructions, "Be strict.");
        assert_eq!(tool.chosen(&json!({ "subagent_type": "general-purpose" })), Ok(None));
        assert!(tool.chosen(&json!({ "subagent_type": "ghost" })).is_err());
        let prompt = system("Parent.", Some(reviewer));
        assert!(prompt.starts_with("Parent.\n\n# You are a sub-agent"));
        assert!(prompt.ends_with("# Your instructions (reviewer)\nBe strict."));
    }
}
