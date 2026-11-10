//! What one [`Superfik`](crate::Superfik) can ask besides running the
//! agent: models, LLM providers, a project's memory, skills and agents.

use std::path::{Path, PathBuf};

use superfik_core::memory::{Memory, MemoryEdit, MemoryId, MemoryKind};
use superfik_core::provider::{ModelProvider, ProviderSummary};
use superfik_core::{CustomAgent, ModelOption, Skill};
use superfik_protocol::{ClientCommand, ProviderEvent, ServerEvent};

use crate::{Error, Superfik};

impl Superfik {
    /// The models the connected providers (and local MLX) offer.
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

    /// Signs in to a subscription (Claude or ChatGPT) in the browser:
    /// `open` gets the URL to visit; returns once signed in.
    pub fn sign_in(&self, provider: ModelProvider, mut open: impl FnMut(&str)) -> Result<Vec<ProviderSummary>, Error> {
        self.client.call(ClientCommand::SignInProvider { provider }, |event| {
            if let ServerEvent::Providers(ProviderEvent::AuthorizeAt { url, .. }) = &event {
                open(url);
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

    /// The memory of the project at `cwd`.
    pub fn memory(&self, cwd: impl Into<PathBuf>) -> ProjectMemory<'_> {
        ProjectMemory { superfik: self, path: cwd.into() }
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
    superfik: &'a Superfik,
    path: PathBuf,
}

impl ProjectMemory<'_> {
    pub fn list(&self) -> Result<Vec<Memory>, Error> {
        self.ask(ClientCommand::LoadMemory { path: self.path.clone() })
    }

    /// The memories that best match `query`, best first, with their score.
    pub fn search(&self, query: &str) -> Result<Vec<(Memory, f32)>, Error> {
        let path = self.path.clone();
        let scores = self.superfik.client.call(
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

    fn ask(&self, command: ClientCommand) -> Result<Vec<Memory>, Error> {
        let path = self.path.clone();
        self.superfik.client.call(command, |event| match event {
            ServerEvent::Memory { path: answered, memories, .. } if answered == path => Some(Ok(memories)),
            _ => None,
        })
    }
}
