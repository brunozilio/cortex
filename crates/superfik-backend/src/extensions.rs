//! What the host program adds to every run (the SDK): folders of skills,
//! commands and agents that keep their own names,
//! MCP servers over HTTP or stdio, and in-process MCP servers. Also what a
//! session in a folder can use, found the way the engine finds it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use superfik_core::mcp::LocalMcp;
use superfik_core::{CustomAgent, ModelOption, PromptRequest, RunId, Skill};

/// An MCP server the agent reaches over streamable HTTP or by starting a
/// process that speaks stdio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerSpec {
    Http { name: String, url: String, headers: BTreeMap<String, String> },
    Stdio { name: String, command: String, args: Vec<String>, env: BTreeMap<String, String> },
}

impl McpServerSpec {
    pub fn name(&self) -> &str {
        match self {
            Self::Http { name, .. } | Self::Stdio { name, .. } => name,
        }
    }

    fn config(&self) -> Value {
        match self {
            Self::Http { url, headers, .. } => json!({ "type": "http", "url": url, "headers": headers }),
            Self::Stdio { command, args, env, .. } => {
                json!({ "type": "stdio", "command": command, "args": args, "env": env })
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Extensions {
    pub dirs: Vec<PathBuf>,
    pub mcp: Vec<McpServerSpec>,
    pub local: Vec<LocalMcp>,
    /// Installed plugins' folders (Claude Code's layout, ADR 0019).
    pub plugins: Vec<PathBuf>,
}

impl Extensions {
    fn for_run(&self) -> superfik_core::Extensions {
        superfik_core::Extensions { dirs: self.dirs.clone(), plugins: self.plugins.clone(), ..Default::default() }
    }

    /// Adds the host's folders and MCP servers to a run and hands it the
    /// in-process servers. The HTTP and stdio servers go in a private
    /// config file the run reads and [`discard`] removes.
    pub(crate) fn brief(&self, run: RunId, request: PromptRequest) -> Result<PromptRequest, String> {
        let mut extensions = self.for_run();
        if !self.mcp.is_empty() {
            let servers: serde_json::Map<String, Value> =
                self.mcp.iter().map(|spec| (spec.name().to_owned(), spec.config())).collect();
            let path = std::env::temp_dir().join(format!("superfik-mcp-{run}.json"));
            let text = json!({ "mcpServers": servers }).to_string();
            superfik_private_file::write(&path, text.as_bytes()).map_err(|error| error.to_string())?;
            extensions.mcp_config = Some(path);
        }
        let mut credentials = request.credentials.clone();
        // `superfik_…` names are Superfik's own servers (its memory search).
        credentials.mcp.extend(self.local.iter().filter(|server| !server.name.starts_with("superfik_")).cloned());
        Ok(PromptRequest { extensions: (!extensions.is_empty()).then_some(extensions), credentials, ..request })
    }
}

/// Removes a finished run's private MCP config.
pub(crate) fn discard(extensions: Option<&superfik_core::Extensions>) {
    if let Some(path) = extensions.and_then(|extensions| extensions.mcp_config.as_ref()) {
        let _ = std::fs::remove_file(path);
    }
}

fn session(cwd: &Path, extensions: &Extensions) -> PromptRequest {
    let mut request = PromptRequest::plain(cwd.to_path_buf(), "", ModelOption::new("", ""));
    request.extensions = Some(extensions.for_run());
    request
}

/// The skills and commands a session in `cwd` can use.
pub(crate) fn skills(cwd: &Path, extensions: &Extensions) -> Vec<Skill> {
    let home = std::env::home_dir();
    superfik_native::tools::skill::discover(&session(cwd, extensions), home.as_deref())
        .into_iter()
        .map(|found| Skill { name: found.name, description: found.description })
        .collect()
}

/// The agents a session in `cwd` can hand work to.
pub(crate) fn agents(cwd: &Path, extensions: &Extensions) -> Vec<CustomAgent> {
    let home = std::env::home_dir();
    superfik_native::tools::subagent::discover(&session(cwd, extensions), home.as_deref())
        .into_iter()
        .map(|found| CustomAgent { name: found.name, description: found.description })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_installed_plugin_s_commands_are_offered_under_its_name() {
        let dir = tempfile::tempdir().expect("dir");
        let plugin = dir.path().join("acme");
        std::fs::create_dir_all(plugin.join("commands")).expect("commands");
        std::fs::write(plugin.join("commands/ship.md"), "---\ndescription: Ships\n---\nShip it.").expect("command");
        let extensions = Extensions { plugins: vec![plugin], ..Extensions::default() };
        let names: Vec<String> = skills(dir.path(), &extensions).into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"acme:ship".into()), "{names:?}");
    }

    #[test]
    fn a_folder_s_skills_commands_and_agents_keep_their_own_names() {
        let dir = tempfile::tempdir().expect("dir");
        let root = dir.path().join("sdk");
        std::fs::create_dir_all(root.join("skills/release")).expect("skills");
        std::fs::create_dir_all(root.join("commands")).expect("commands");
        std::fs::create_dir_all(root.join("agents")).expect("agents");
        std::fs::write(
            root.join("skills/release/SKILL.md"),
            "---\nname: release\ndescription: Cuts a release\n---\nSteps",
        )
        .expect("skill");
        std::fs::write(root.join("commands/audit.md"), "---\ndescription: Audits\n---\n$ARGUMENTS").expect("command");
        std::fs::write(root.join("agents/reviewer.md"), "---\nname: reviewer\ndescription: Reviews\n---\nReview.")
            .expect("agent");
        let extensions = Extensions { dirs: vec![root], ..Extensions::default() };
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).expect("project");
        let names: Vec<String> = skills(&project, &extensions).into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"release".into()) && names.contains(&"audit".into()), "{names:?}");
        assert!(agents(&project, &extensions).iter().any(|a| a.name == "reviewer"));
    }

    struct Silent;

    impl superfik_core::mcp::McpEndpoint for Silent {
        fn handle(&self, _: &Value) -> Option<Value> {
            None
        }
    }

    #[test]
    fn a_run_gets_the_host_s_folders_servers_and_in_process_tools() {
        let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
        let run = RunId::new();
        assert_eq!(Extensions::default().brief(run, request.clone()).expect("brief"), request, "nothing to add");
        let extensions = Extensions {
            dirs: vec!["/sdk".into()],
            mcp: vec![McpServerSpec::Http {
                name: "docs".into(),
                url: "https://docs.test/mcp".into(),
                headers: [("Authorization".to_owned(), "Bearer t".to_owned())].into(),
            }],
            local: vec![LocalMcp::new("host", std::sync::Arc::new(Silent))],
            plugins: vec!["/plugins/acme".into()],
        };
        let briefed = extensions.brief(run, request).expect("brief");
        let added = briefed.extensions.clone().expect("extensions");
        assert_eq!(added.dirs, [PathBuf::from("/sdk")]);
        assert_eq!(added.plugins, [PathBuf::from("/plugins/acme")]);
        let path = added.mcp_config.clone().expect("config");
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(config["mcpServers"]["docs"]["headers"]["Authorization"], "Bearer t");
        assert_eq!(briefed.credentials.mcp.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["host"]);
        discard(Some(&added));
        assert!(!path.exists());
    }
}
