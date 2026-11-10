//! What a run reads from disk before it starts: skills, agents, settings,
//! rules, git state, project instructions and installed plugins.
use std::path::PathBuf;

use crate::{prompt, tools};
use serde::{Deserialize, Serialize};
use superfik_core::PromptRequest;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brief {
    pub skills: Vec<tools::skill::FoundSkill>,
    pub agents: Vec<tools::subagent::FoundAgent>,
    pub settings: prompt::settings::Settings,
    pub rules: Vec<prompt::rules::Rule>,
    pub git: Option<prompt::GitSnapshot>,
    pub instructions: Vec<prompt::Instructions>,
    #[serde(default)]
    pub plugins: Vec<crate::plugins::Plugin>,
    #[serde(default)]
    pub home: Option<PathBuf>,
}
impl Brief {
    pub fn load(request: &PromptRequest) -> Result<Self, String> {
        let home = home(request);
        let home = home.as_deref();
        let plugins = crate::plugins::found(request, home);
        Ok(Self {
            skills: tools::skill::discover(request, home),
            agents: tools::subagent::discover(request, home),
            settings: prompt::settings::load(&request.cwd, home, &plugins)?,
            rules: prompt::rules::discover(&request.cwd, home),
            git: prompt::snapshot(&request.cwd),
            instructions: prompt::instructions(&request.cwd, home),
            plugins,
            home: home.map(PathBuf::from),
        })
    }

    /// Where the run's configured MCP servers come from.
    pub fn mcp(&self) -> crate::mcp::McpSources {
        crate::mcp::McpSources {
            home: self.home.clone(),
            plugins: self.plugins.clone(),
            approval: self.settings.mcp_approval.clone(),
        }
    }
}

/// The owner's home: the one the backend named for the run, else the
/// process's.
pub fn home(request: &PromptRequest) -> Option<PathBuf> {
    request.extensions.as_ref().and_then(|extensions| extensions.home.clone()).or_else(std::env::home_dir)
}
