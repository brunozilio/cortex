use std::sync::Arc;

use domain::SessionId;

use super::equip::{MainAgent, Toolbox};
use super::runtime::{Runtime, SessionWorktree};
use crate::agent::Agent;
use crate::agent_runtime::Setup;
use crate::brief::Brief;
use crate::engine::MAX_OUTPUT;
use crate::engine::date::{time_zone, today};
use crate::engine::job::Job;
use crate::engine::stop::Stop;
use crate::model::TurnRequest;
use crate::prompt::{self, Environment};
use crate::tools::{self, Workspace};

pub(super) struct Framed {
    pub(super) workspace: Workspace,
    pub(super) toolbox: Toolbox,
    pub(super) main: MainAgent,
    pub(super) worktree: SessionWorktree,
    pub(super) template: TurnRequest,
    pub(super) extra: String,
}

impl Job {
    pub(super) fn frame(&self, runtime: Runtime, brief: &Brief, session: &SessionId) -> Result<Framed, Stop> {
        let Runtime { mut workspace, mut toolbox, main, worktree } = runtime;
        let setup = &worktree.lease.setup;
        let environment = self.environment(&workspace, &toolbox, setup, brief, session);
        let mut system = prompt::system(&environment);
        workspace.prompt_context = Some(environment);
        let extra = self.extra_instructions(&main, setup, &toolbox, &workspace)?;
        let offered = Agent::specs(&toolbox.tools).into_iter().filter(|spec| main.config.allows(&spec.name)).collect();
        let (offered, deferred) = tools::tool_search::defer(offered);
        if let Some(deferred) = deferred {
            toolbox.tools.push(Box::new(tools::tool_search::ToolSearch));
            workspace.services.deferred = Some(Arc::new(deferred));
        }
        system.push_str(&extra);
        let template = TurnRequest {
            model: self.request.session.model.id.clone(),
            system,
            messages: Vec::new(),
            tools: offered,
            effort: self.request.session.effort,
            max_tokens: MAX_OUTPUT,
        };
        Ok(Framed { workspace, toolbox, main, worktree, template, extra })
    }

    fn environment(
        &self,
        workspace: &Workspace,
        toolbox: &Toolbox,
        setup: &Setup,
        brief: &Brief,
        session: &SessionId,
    ) -> Environment {
        let moved_to = setup.context.as_ref();
        let git = moved_to.and_then(|context| context.git.clone()).or_else(|| brief.git.clone());
        let snapshot = self.session_snapshot(session, || prompt::Snapshot {
            date: today(),
            time_zone: time_zone(),
            git,
            top_level: prompt::top_level(&workspace.cwd),
        });
        if let Some(recorder) = &self.recorder {
            recorder.opened(self.run_info(&workspace.cwd, snapshot.git.as_ref()));
        }
        let language_servers = workspace.services.lsp.as_ref();
        Environment {
            cwd: workspace.cwd.clone(),
            added_dirs: workspace.added_dirs.clone(),
            platform: prompt::platform(),
            date: snapshot.date,
            time_zone: snapshot.time_zone,
            top_level: snapshot.top_level,
            model: self.request.session.model.label.clone(),
            language_servers: language_servers
                .map(|lsp| lsp.available().iter().map(prompt::LanguageServer::of).collect())
                .unwrap_or_default(),
            git: snapshot.git,
            instructions: moved_to.map_or_else(|| brief.instructions.clone(), |context| context.instructions.clone()),
            tools: toolbox.tools.iter().map(|tool| tool.spec().name).collect(),
            skills: tools::skill::listing(&brief.skills),
            output_style: prompt::styles::picked(
                self.request.session.output_style.as_deref(),
                brief.settings.style.clone(),
            ),
            mcp_instructions: toolbox
                .servers
                .iter()
                .filter_map(|server| {
                    server.instructions().map(|text| prompt::McpInstructions::of(server.name(), &text))
                })
                .collect(),
        }
    }

    fn extra_instructions(
        &self,
        main: &MainAgent,
        setup: &Setup,
        toolbox: &Toolbox,
        workspace: &Workspace,
    ) -> Result<String, Stop> {
        let mut extra = String::new();
        if let Some(agent) = &main.found {
            extra.push_str(&format!("\n\n# Agent instructions ({})\n{}", agent.name, agent.instructions));
        }
        if let Some(appended) = self.request.session.append_system.as_deref().filter(|text| !text.trim().is_empty()) {
            extra.push_str(&format!("\n\n{}", appended.trim()));
        }
        if let Some(format) = self.request.host.memory.as_ref().and_then(domain::MemoryBrief::edits_format) {
            extra.push_str(&format!("\n\n{format}"));
        }
        if let Some(dir) = &setup.memory_dir {
            extra.push_str(&format!(
                "\n\n# Persistent agent memory\nRead and update MEMORY.md in {}. Keep its index under 200 lines and 25 KB.\n{}",
                dir.display(),
                setup.memory
            ));
        }
        for skill in &main.config.skills {
            let Some(tool) = toolbox.tools.iter().find(|tool| tool.spec().name == crate::tools::names::SKILL) else {
                return Err(Stop::failed("Configured agent requires the Skill tool"));
            };
            let loaded = tool.run(&serde_json::json!({"skill":skill}), workspace);
            if loaded.is_error {
                return Err(Stop::failed(loaded.content));
            }
            extra.push_str(&format!("\n\n# Preloaded skill\n{}", loaded.content));
        }
        Ok(extra)
    }
}
