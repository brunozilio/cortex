//! What one [`Sdk`](crate::Sdk) can ask besides running the
//! agent: models, LLM providers, a project's memory, skills and agents.

use std::path::{Path, PathBuf};

use domain::memory::{Forgotten, Memory, MemoryEdit, MemoryId, MemoryKind};
use domain::provider::{ModelProvider, ProviderSummary};
use domain::{CustomAgent, ModelOption, Skill};
use protocol::{ClientCommand, ProviderEvent, ServerEvent};

use crate::{Error, Run, Sdk};

impl Sdk {
    /// The models the connected providers offer.
    pub fn models(&self) -> Result<Vec<ModelOption>, Error> {
        self.client.call(ClientCommand::ListModels, |event| match event {
            ServerEvent::Models(models) => Some(Ok(models)),
            _ => None,
        })
    }

    pub fn providers(&self) -> Result<Vec<ProviderSummary>, Error> {
        self.client.call(ClientCommand::ListProviders, listed)
    }

    /// Connects a provider with an API key (or a subscription's token JSON).
    pub fn connect_provider(&self, provider: ModelProvider, key: &str) -> Result<Vec<ProviderSummary>, Error> {
        self.client.call(ClientCommand::ConnectProvider { provider, key: key.to_owned() }, listed)
    }

    /// Signs in to a subscription (Claude, ChatGPT or GitHub Copilot) in
    /// the browser: `open` gets the URL to visit and, for Copilot, the code
    /// to type there; returns once signed in.
    pub fn sign_in(
        &self,
        provider: ModelProvider,
        mut open: impl FnMut(&str, Option<&str>),
    ) -> Result<Vec<ProviderSummary>, Error> {
        self.client.call(ClientCommand::SignInProvider { provider }, |event| {
            if let ServerEvent::Providers(ProviderEvent::AuthorizeAt { url, code, .. }) = &event {
                open(url, code.as_deref());
                return None;
            }
            listed(event)
        })
    }

    pub fn disconnect_provider(&self, provider: ModelProvider) -> Result<Vec<ProviderSummary>, Error> {
        self.client.call(ClientCommand::DisconnectProvider { provider }, listed)
    }

    /// The skills and commands a session in `cwd` can use.
    pub fn skills(&self, cwd: &Path) -> Result<Vec<Skill>, Error> {
        self.client.call(ClientCommand::ListSkills { path: cwd.to_path_buf() }, |event| match event {
            ServerEvent::Skills(skills) => Some(Ok(skills)),
            _ => None,
        })
    }

    /// The agents a session in `cwd` can hand work to.
    pub fn agents(&self, cwd: &Path) -> Result<Vec<CustomAgent>, Error> {
        self.client.call(ClientCommand::ListAgents { path: cwd.to_path_buf() }, |event| match event {
            ServerEvent::Agents(agents) => Some(Ok(agents)),
            _ => None,
        })
    }

    /// This owner's settings (a JSON object; empty when none were saved).
    pub fn settings(&self) -> Result<serde_json::Value, Error> {
        self.client.call(ClientCommand::LoadSettings, |event| match event {
            ServerEvent::Settings { settings } => Some(Ok(settings.unwrap_or_else(|| serde_json::json!({})))),
            _ => None,
        })
    }

    /// Replaces this owner's settings.
    pub fn save_settings(&self, settings: serde_json::Value) -> Result<(), Error> {
        self.client.send(ClientCommand::SaveSettings { settings })
    }

    /// The memory of the project at `cwd`.
    pub fn memory(&self, cwd: impl Into<PathBuf>) -> ProjectMemory<'_> {
        ProjectMemory { sdk: self, path: cwd.into() }
    }
}

fn listed(event: ServerEvent) -> Option<Result<Vec<ProviderSummary>, String>> {
    match event {
        ServerEvent::Providers(ProviderEvent::Listed { providers }) => Some(Ok(providers)),
        ServerEvent::Providers(ProviderEvent::Failed { message, .. }) => Some(Err(message)),
        _ => None,
    }
}

/// What agents remember about one project between sessions.
#[derive(Debug)]
pub struct ProjectMemory<'a> {
    sdk: &'a Sdk,
    path: PathBuf,
}

impl ProjectMemory<'_> {
    pub fn list(&self) -> Result<Vec<Memory>, Error> {
        self.ask(ClientCommand::LoadMemory { path: self.path.clone() })
    }

    /// The memories that best match `query`, best first, with their score.
    pub fn search(&self, query: &str) -> Result<Vec<(Memory, f32)>, Error> {
        let path = self.path.clone();
        let scores = self.sdk.client.call(
            ClientCommand::SearchMemory { path: path.clone(), query: query.to_owned() },
            |event| match event {
                ServerEvent::MemorySearch { path: answered, scores, .. } if answered == path => Some(Ok(scores)),
                _ => None,
            },
        )?;
        let memories = self.list()?;
        Ok(scores
            .into_iter()
            .filter_map(|(id, score)| Some((memories.iter().find(|m| m.id == id)?.clone(), score)))
            .collect())
    }

    pub fn remember(&self, kind: MemoryKind, text: &str, tags: &[&str]) -> Result<Vec<Memory>, Error> {
        let tags = tags.iter().map(|tag| (*tag).to_owned()).collect();
        self.edit(vec![MemoryEdit::Remember { kind, text: text.to_owned(), tags }])
    }

    pub fn forget(&self, id: MemoryId) -> Result<Vec<Memory>, Error> {
        self.edit(vec![MemoryEdit::Forget { id }])
    }

    pub fn revise(&self, id: MemoryId, text: &str) -> Result<Vec<Memory>, Error> {
        self.edit(vec![MemoryEdit::Revise { id, text: text.to_owned() }])
    }

    pub fn edit(&self, edits: Vec<MemoryEdit>) -> Result<Vec<Memory>, Error> {
        self.ask(ClientCommand::EditMemory { path: self.path.clone(), edits })
    }

    /// Brings back a memory forgotten in the last 30 days.
    pub fn restore(&self, id: MemoryId) -> Result<Vec<Memory>, Error> {
        self.edit(vec![MemoryEdit::Restore { id }])
    }

    /// What was forgotten and can still be restored, oldest first.
    pub fn forgotten(&self) -> Result<Vec<Forgotten>, Error> {
        self.answer(ClientCommand::LoadMemory { path: self.path.clone() }).map(|(_, forgotten)| forgotten)
    }

    /// Starts a run in which an agent tidies this memory with `model`:
    /// merges, corrects, reclassifies and drops. [`ProjectMemory::undo`]
    /// puts it back as it was until the next edit.
    pub fn optimize(&self, model: ModelOption) -> Result<Run, Error> {
        let path = self.path.clone();
        Run::start(&self.sdk.client, |run| ClientCommand::OptimizeMemory { run, path, model })
    }

    /// The memory as it was before the last optimization.
    pub fn undo(&self) -> Result<Vec<Memory>, Error> {
        self.ask(ClientCommand::UndoMemory { path: self.path.clone() })
    }

    fn ask(&self, command: ClientCommand) -> Result<Vec<Memory>, Error> {
        self.answer(command).map(|(memories, _)| memories)
    }

    fn answer(&self, command: ClientCommand) -> Result<(Vec<Memory>, Vec<Forgotten>), Error> {
        let path = self.path.clone();
        self.sdk.client.call(command, |event| match event {
            ServerEvent::Memory { path: answered, memories, forgotten } if answered == path => {
                Some(Ok((memories, forgotten)))
            }
            _ => None,
        })
    }
}
