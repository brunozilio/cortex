use domain::SessionId;

use crate::engine::job::Job;
use crate::engine::stop::Stop;
use crate::meter::Metered;
use crate::model::TurnRequest;
use crate::tools::Workspace;
use crate::tools::skill::{self, FoundSkill};
use crate::tools::subagent_config::AgentConfig;
use crate::transcript::{Block, Message};

#[derive(Clone, Copy)]
pub(super) struct Opening<'a> {
    pub(super) api: &'a Metered,
    pub(super) template: &'a TurnRequest,
    pub(super) workspace: &'a Workspace,
    pub(super) session: &'a SessionId,
}

impl Job {
    pub(super) fn user_message(
        &self,
        config: &AgentConfig,
        skills: &[FoundSkill],
        workspace: &Workspace,
        first_prompt: bool,
    ) -> Result<Message, Stop> {
        let initial = if first_prompt { config.initial_prompt.as_deref().unwrap_or_default() } else { "" };
        let cwd = &self.request.session.cwd;
        let initial = skill::expand_initial(initial, skills, cwd, workspace).map_err(Stop::failed)?;
        let typed = match skill::expand_command(&self.request.prompt.text, skills, cwd, workspace) {
            Some(expanded) => self.request.composed(&expanded.map_err(Stop::failed)?),
            None => self.request.compose(),
        };
        let mut user = Message::user(format!("{}{}{}", initial, if initial.is_empty() { "" } else { "\n\n" }, typed));
        user.blocks.splice(0..0, self.request.prompt.images.iter().cloned().map(Block::Image));
        if let Some(reminder) = self.request.host.memory.as_ref().and_then(domain::MemoryBrief::edits_reminder) {
            user.blocks.push(Block::Reminder(reminder));
        }
        Ok(user)
    }

    pub(super) fn submitted(&self, mut user: Message, opening: &Opening<'_>) -> Result<Message, Stop> {
        let Opening { api, template, workspace, session } = *opening;
        let announced = self.announced(workspace, session, &user);
        for (event, subject, payload) in announced {
            let hook = workspace
                .hooks
                .run_with_model(event, subject, payload, workspace, Some((api, template)))
                .map_err(Stop::failed)?;
            if let Some(blocked) = hook.blocked {
                return Err(Stop::failed(blocked));
            }
            if let Some(prompt) = hook.prompt
                && let Some(typed) = user.blocks.get_mut(self.request.prompt.images.len())
            {
                *typed = Block::Text(prompt);
            }
            if !hook.context.is_empty() {
                user.blocks.push(Block::Text(hook.context));
            }
        }
        Ok(user)
    }
}
