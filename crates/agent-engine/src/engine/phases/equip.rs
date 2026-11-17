use std::sync::Arc;

use crate::brief::Brief;
use crate::engine::job::Job;
use crate::engine::stop::Stop;
use crate::mcp::McpServer;
use crate::tools::subagent::{AgentTool, FoundAgent};
use crate::tools::subagent_config::AgentConfig;
use crate::tools::{self, Tool, Workspace};

pub(in crate::engine) struct Toolbox {
    pub(in crate::engine) tools: Vec<Box<dyn Tool>>,
    _kept_connected_for_the_run: Vec<Arc<McpServer>>,
}

pub(in crate::engine) struct MainAgent {
    pub(in crate::engine) found: Option<FoundAgent>,
    pub(in crate::engine) config: AgentConfig,
}

impl MainAgent {
    pub(in crate::engine) fn chosen(brief: &Brief) -> Result<Self, String> {
        let found = match &brief.settings.agent {
            Some(name) => {
                AgentTool { agents: brief.agents.clone() }.chosen(&serde_json::json!({"subagent_type":name}))?.cloned()
            }
            None => None,
        };
        let mut config = found.as_ref().map(|agent| agent.config.clone()).unwrap_or_default();
        if !brief.settings.auto_memory {
            config.memory = None;
        }
        Ok(Self { found, config })
    }

    pub(in crate::engine) fn name(&self) -> &str {
        self.found.as_ref().map_or("main", |agent| agent.name.as_str())
    }
}

pub(in crate::engine) struct Equipped {
    pub(in crate::engine) workspace: Workspace,
    pub(in crate::engine) toolbox: Toolbox,
    pub(in crate::engine) main: MainAgent,
}

impl Job {
    pub(in crate::engine) fn equip(&mut self, brief: &Brief, mut workspace: Workspace) -> Result<Equipped, Stop> {
        workspace.rules.clone_from(&brief.rules);
        let mut tools = tools::standard();
        self.offer_language_servers(brief, &mut workspace, &mut tools);
        if !brief.skills.is_empty() {
            tools.push(Box::new(tools::skill::SkillTool { skills: brief.skills.clone() }));
        }
        tools.push(Box::new(AgentTool { agents: brief.agents.clone() }));
        tools.push(Box::new(tools::send_message::SendMessageTool));
        tools.push(Box::new(tools::repl::Repl));
        tools.extend(tools::task::tools());
        let stop = self.control.stop_flag();
        let servers = crate::mcp::connect_stopped(&self.request, &brief.mcp(), &self.mcp, stop);
        tools.extend(crate::mcp::tools_of(&servers, stop));
        tools::script_tool::join(&mut tools, tools::script_tool::load(&brief.gate(&self.request.cwd)));
        if let Some(expanded) = crate::mcp::extras::expand_prompt(&servers, &self.request.prompt, stop) {
            self.request.prompt = expanded.map_err(Stop::failed)?;
        }
        let main = MainAgent::chosen(brief).map_err(Stop::failed)?;
        self.control = self.control.reconfigured(main.config.permission, main.config.dont_ask);
        workspace.stop = self.control.shared_stop();
        Ok(Equipped { workspace, toolbox: Toolbox { tools, _kept_connected_for_the_run: servers }, main })
    }

    fn offer_language_servers(&self, brief: &Brief, workspace: &mut Workspace, tools: &mut Vec<Box<dyn Tool>>) {
        let specs = domain::lsp::servers(domain::lsp::defaults(), &brief.settings.lsp_servers);
        let lsp = crate::lsp::Lsp { servers: Arc::clone(&self.lsp), specs: Arc::new(specs) };
        if !lsp.available().is_empty() {
            workspace.services.lsp = Some(lsp);
            tools.push(Box::new(tools::Lsp));
        }
    }
}
