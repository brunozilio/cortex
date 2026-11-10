//! What the host program adds to every run (the SDK, ADR 0018): folders
//! laid out like a plugin whose skills, commands and agents keep their own
//! names, MCP servers over HTTP or stdio, and in-process MCP servers.

use std::path::{Path, PathBuf};

use superfik_core::mcp::LocalMcp;
use superfik_core::{CustomAgent, PromptRequest, RunId, Skill};
use superfik_plugins::McpServerSpec;
use superfik_plugins::agents::agent_from_markdown;
use superfik_plugins::skills::skill_from_markdown;

#[derive(Debug, Clone, Default)]
pub struct Extensions {
    pub dirs: Vec<PathBuf>,
    pub mcp: Vec<McpServerSpec>,
    pub local: Vec<LocalMcp>,
}

impl Extensions {
    /// Skills (`skills/<name>/SKILL.md`) and commands (`commands/<name>.md`).
    pub fn skills(&self) -> Vec<Skill> {
        let mut skills = Vec::new();
        for dir in &self.dirs {
            for (stem, text) in markdown(&dir.join("skills"), true) {
                skills.push(skill_from_markdown(&stem, &text));
            }
            for (stem, text) in markdown(&dir.join("commands"), false) {
                skills.push(skill_from_markdown(&stem, &text));
            }
        }
        skills
    }

    /// Adds the host's folders and MCP servers to a run already briefed
    /// with its plugins, and hands it the in-process servers. The HTTP and
    /// stdio servers join the run's private MCP config file.
    pub(crate) fn brief(&self, run: RunId, request: PromptRequest) -> Result<PromptRequest, String> {
        let mut brief = request.plugins.clone().unwrap_or_default();
        brief.local_dirs.extend(self.dirs.iter().cloned());
        if !self.mcp.is_empty() {
            let path = brief.mcp_config.clone().unwrap_or_else(|| crate::plugins::mcp_path(run));
            let mut config = match std::fs::read_to_string(&path) {
                Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string())?,
                Err(_) => serde_json::json!({ "mcpServers": {} }),
            };
            for spec in &self.mcp {
                config["mcpServers"][spec.name()] = superfik_plugins::mcp_config::plain(spec);
            }
            crate::plugins::write_private(&path, &config.to_string()).map_err(|error| error.to_string())?;
            brief.mcp_config = Some(path);
        }
        let mut credentials = request.credentials.clone();
        credentials.mcp.extend(self.local.iter().cloned());
        Ok(PromptRequest { plugins: (!brief.is_empty()).then_some(brief), credentials, ..request })
    }

    /// Agents (`agents/<name>.md`).
    pub fn agents(&self) -> Vec<CustomAgent> {
        self.dirs
            .iter()
            .flat_map(|dir| markdown(&dir.join("agents"), false))
            .filter_map(|(stem, text)| agent_from_markdown(&stem, &text, None))
            .collect()
    }
}

/// `(name, text)` of each `<name>.md`, or of each `<name>/SKILL.md` when
/// `folders`, sorted by name.
fn markdown(dir: &Path, folders: bool) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<(String, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let (stem, file) = if folders {
                (path.file_name()?.to_string_lossy().into_owned(), path.join("SKILL.md"))
            } else if path.extension().is_some_and(|ext| ext == "md") {
                (path.file_stem()?.to_string_lossy().into_owned(), path)
            } else {
                return None;
            };
            Some((stem, std::fs::read_to_string(file).ok()?))
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_lists_its_skills_commands_and_agents_under_their_own_names() {
        let dir = tempfile::tempdir().expect("dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("skills/release")).expect("skills");
        std::fs::create_dir_all(root.join("commands")).expect("commands");
        std::fs::create_dir_all(root.join("agents")).expect("agents");
        std::fs::write(root.join("skills/release/SKILL.md"), "---\ndescription: Cuts a release\n---\nSteps")
            .expect("skill");
        std::fs::write(root.join("commands/audit.md"), "---\ndescription: Audits\n---\n$ARGUMENTS").expect("command");
        std::fs::write(root.join("agents/reviewer.md"), "---\nname: reviewer\ndescription: Reviews\n---\nReview.")
            .expect("agent");
        let extensions = Extensions { dirs: vec![root.to_path_buf()], ..Extensions::default() };
        let skills: Vec<_> = extensions.skills().into_iter().map(|s| (s.name, s.description)).collect();
        assert_eq!(skills, [("release".into(), "Cuts a release".into()), ("audit".into(), "Audits".into())]);
        assert_eq!(extensions.agents().iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["reviewer"]);
    }

    struct Silent;

    impl superfik_core::mcp::McpEndpoint for Silent {
        fn handle(&self, _: &serde_json::Value) -> Option<serde_json::Value> {
            None
        }
    }

    #[test]
    fn a_run_gets_the_host_s_folders_servers_and_in_process_tools() {
        let model = superfik_core::ModelOption::new(superfik_core::EngineKind::Native, "m", "M");
        let request = PromptRequest::plain("/repo".into(), "hi", model);
        let run = RunId::new();
        assert_eq!(Extensions::default().brief(run, request.clone()).expect("brief"), request, "nothing to add");
        let extensions = Extensions {
            dirs: vec!["/sdk".into()],
            mcp: vec![McpServerSpec::Http {
                name: "docs".into(),
                url: "https://docs.test/mcp".into(),
                headers: [("Authorization".to_owned(), "Bearer {{x}}".to_owned())].into(),
            }],
            local: vec![LocalMcp::new("host", std::sync::Arc::new(Silent))],
        };
        let briefed = extensions.brief(run, request).expect("brief");
        let brief = briefed.plugins.clone().expect("brief");
        assert_eq!(brief.local_dirs, [PathBuf::from("/sdk")]);
        let path = brief.mcp_config.clone().expect("config");
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(config["mcpServers"]["docs"]["headers"]["Authorization"], "Bearer {{x}}", "written as given");
        assert_eq!(briefed.credentials.mcp.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["host"]);
        crate::plugins::discard(Some(&brief));
    }
}
