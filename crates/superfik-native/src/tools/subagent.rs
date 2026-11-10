//! `Agent`: hands a self-contained task to a sub-agent, which works with
//! the same tools in its own conversation and returns a report. The agent
//! loop runs it (it needs the model); here are its schema and the custom
//! agents (`<name>.md` with `name` and `description` frontmatter, the body
//! being the sub-agent's instructions) it can be given.

use std::path::{Path, PathBuf};

use super::subagent_config::AgentConfig;
use crate::claude_home::ClaudeHome;
use serde_json::{Value, json};
use superfik_core::PromptRequest;

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
        sources.extend([(directory.join(".superfik/agents"), None), (directory.join(".claude/agents"), None)]);
    }
    if let Some(home) = home {
        sources.push((home.join(".superfik/agents"), None));
        let claude = ClaudeHome::new(home.join(".claude"));
        sources.extend(claude.agent_sources().into_iter().map(|source| (source.dir, source.namespace)));
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
        let mut description = String::from(AGENT_GUIDE);
        description.push_str(
            "\n\nAgents available through `subagent_type`:\n- general-purpose: researching complex questions, \
             searching for code and carrying out tasks of several steps (the default).",
        );
        for agent in &self.agents {
            description.push_str(&format!("\n- {}: {}", agent.name, agent.description));
        }
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

const AGENT_GUIDE: &str = "\
Starts a sub-agent for a task of several steps. Each kind of agent has its own abilities and tools. Leaving out \
`subagent_type` starts a fresh general-purpose agent; `fork` forks you, inheriting your whole conversation and \
your model.

When to use it: a fresh agent costs more than it looks. It knows only what you put in `prompt`, and you see only \
the report it sends back: both handoffs drop detail, and neither of you can tell what the other missed. You cannot \
watch it work, only wait or cancel. Its mistakes come back in the same confident tone as its findings, and an \
agent handed your hypothesis tends to return it confirmed. Several at once spend tokens in a burst the user did \
not ask for.

Reach for it when you have independent work to run in parallel, when the user asks for a side task that should \
not block you, or when answering means reading across many files: delegate that and you keep the conclusion, not \
the file dumps. Do the work yourself when it is a handful of tool calls or a lookup whose target you already know. \
Delegate a review only when you want a reading not anchored on yours, and then give it the code, not your \
conclusion. Once you delegated something, do not also do it yourself; wait for the result. When in doubt, do not \
start one.

Brief it like the peer it is: state the goal and what you already ruled out, point it at the files and documents \
worth reading instead of retyping them, and keep the scope explicit and narrow. If you cannot write a clear \
brief, you do not understand the task well enough to hand it off. A fork needs no background, since it has \
yours: give it only its directive.

- Its report is not shown to the user: pass on what matters.
- `run_in_background` runs it while you go on; collect its report with TaskOutput. Never make up or predict a \
pending agent's result; if the user asks before it arrives, say it is still running.
- Each kind's model, effort and tools come from its definition (`.superfik/agents/*.md` frontmatter).";

pub fn system(parent: &str, agent: Option<&FoundAgent>) -> String {
    let mut system = format!(
        "{parent}\n\n# You are a sub-agent\nAnother agent handed you the task in the user's message. Do it with \
         your tools, without asking anyone: no one answers questions here. Finish with a concise report of what \
         you found or did, with the paths that matter; it goes back to that agent, not to the user.\n\n{}",
        super::builtin_agents::SHARED_NOTES
    );
    match agent.filter(|agent| !agent.instructions.is_empty()) {
        Some(agent) => system.push_str(&format!("\n\n# Your instructions ({})\n{}", agent.name, agent.instructions)),
        None if agent.is_none() => {
            system.push_str(&format!("\n\n# Your instructions ({GENERAL})\n{}", super::builtin_agents::GENERAL))
        }
        None => {}
    }
    system
}

#[cfg(test)]
mod tests {
    use superfik_core::ModelOption;

    use super::*;

    #[test]
    fn finds_custom_agents_and_picks_the_one_asked_for() {
        let repo = tempfile::tempdir().expect("repo");
        let dir = repo.path().join(".superfik/agents");
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
