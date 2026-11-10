//! What a run reads from disk before it starts: skills, agents, settings,
//! rules, git state and project instructions.
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
}
impl Brief {
    pub fn load(request: &PromptRequest) -> Result<Self, String> {
        let home = std::env::home_dir();
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
        })
    }
}
