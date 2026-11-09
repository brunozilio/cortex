//! Custom Claude Code sub-agents: `<name>.md` files with `name` and
//! `description` frontmatter under `.claude/agents/`.

use std::fs;
use std::path::{Path, PathBuf};

use superfik_core::CustomAgent;

use crate::claude_home::{ClaudeHome, Source, bare_name, qualified};
use crate::skills::parse_frontmatter;

pub trait AgentCatalog: Send + Sync {
    fn agents(&self, repo: &Path) -> Vec<CustomAgent>;
}

#[derive(Debug, Clone)]
pub struct AgentDiscovery {
    home: Option<ClaudeHome>,
}

impl AgentDiscovery {
    pub fn new() -> Self {
        Self { home: ClaudeHome::detect() }
    }

    pub fn with_home(root: Option<PathBuf>) -> Self {
        Self { home: root.map(ClaudeHome::new) }
    }
}

impl Default for AgentDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentCatalog for AgentDiscovery {
    fn agents(&self, repo: &Path) -> Vec<CustomAgent> {
        let mut sources = vec![Source { dir: repo.join(".claude").join("agents"), namespace: None }];
        sources.extend(self.home.iter().flat_map(|home| home.agent_sources(repo)));
        let mut agents: Vec<CustomAgent> = Vec::new();
        for source in sources {
            for agent in scan(&source) {
                if !agents.iter().any(|known| bare_name(&known.name) == bare_name(&agent.name)) {
                    agents.push(agent);
                }
            }
        }
        agents
    }
}

fn scan(source: &Source) -> Vec<CustomAgent> {
    let Ok(entries) = fs::read_dir(&source.dir) else { return Vec::new() };
    let mut agents: Vec<CustomAgent> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let text = fs::read_to_string(&path).ok()?;
            agent_from_markdown(&stem, &text, source.namespace.as_deref())
        })
        .collect();
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

pub fn agent_from_markdown(stem: &str, text: &str, namespace: Option<&str>) -> Option<CustomAgent> {
    let (front, _) = parse_frontmatter(text);
    let description = front.get("description")?.to_owned();
    let name = front.get("name").unwrap_or(stem).trim();
    Some(CustomAgent { name: qualified(namespace, name), description: first_sentence(&description) })
}

fn first_sentence(text: &str) -> String {
    let text = text.trim();
    match text.find(". ") {
        Some(end) => text[..=end].trim().to_owned(),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, file: &str, text: &str) {
        fs::create_dir_all(dir).expect("mkdir");
        fs::write(dir.join(file), text).expect("write");
    }

    #[test]
    fn project_agents_win_over_user_and_plugin_agents() {
        let repo = tempfile::tempdir().expect("repo");
        let home = tempfile::tempdir().expect("home");
        write(
            &repo.path().join(".claude/agents"),
            "reviewer.md",
            "---\nname: reviewer\ndescription: Repo reviewer\n---\nBody",
        );
        write(&home.path().join("agents"), "reviewer.md", "---\nname: reviewer\ndescription: User reviewer\n---\n");
        write(&home.path().join("agents"), "planner.md", "---\ndescription: Plans work. Use it early.\n---\n");
        write(&home.path().join("agents"), "notes.md", "# just notes\n");
        let plugin = home.path().join("cache/sec/1.0");
        write(&plugin.join("agents"), "scanner.md", "---\nname: scanner\ndescription: Scans\n---\n");
        let installed =
            format!(r#"{{"version":2,"plugins":{{"sec@m":[{{"installPath":{}}}]}}}}"#, serde_json::json!(plugin));
        write(&home.path().join("plugins"), "installed_plugins.json", &installed);

        let agents = AgentDiscovery::with_home(Some(home.path().to_path_buf())).agents(repo.path());
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["reviewer", "planner", "sec:scanner"]);
        assert_eq!(agents[0].description, "Repo reviewer");
        assert_eq!(agents[1].description, "Plans work.");
    }

    #[test]
    fn nothing_without_directories() {
        assert!(AgentDiscovery::with_home(None).agents(Path::new("/definitely/not/here")).is_empty());
    }
}
