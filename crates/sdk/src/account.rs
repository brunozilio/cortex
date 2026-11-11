use std::path::{Path, PathBuf};

use domain::memory::{Forgotten, Memory, MemoryEdit, MemoryId, MemoryKind};
use domain::provider::{ModelProvider, ProviderSummary};
use domain::{CustomAgent, ModelOption, Skill};
use protocol::{ClientCommand, ProviderEvent, ServerEvent};
use serde_json::{Value, json};

use crate::{Error, Run, Sdk};

impl Sdk {
    pub fn models(&self) -> Result<Vec<ModelOption>, Error> {
        self.client.call(ClientCommand::ListModels, |event| match event {
            ServerEvent::Models(models) => Some(Ok(models)),
            _ => None,
        })
    }

    pub fn providers(&self) -> Result<Vec<ProviderSummary>, Error> {
        self.client.call(ClientCommand::ListProviders, listed)
    }

    pub fn connect_provider(&self, provider: ModelProvider, key: &str) -> Result<Vec<ProviderSummary>, Error> {
        self.client.call(ClientCommand::ConnectProvider { provider, key: key.to_owned() }, listed)
    }

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

    pub fn skills(&self, cwd: &Path) -> Result<Vec<Skill>, Error> {
        self.client.call(ClientCommand::ListSkills { path: cwd.to_path_buf() }, |event| match event {
            ServerEvent::Skills(skills) => Some(Ok(skills)),
            _ => None,
        })
    }

    pub fn agents(&self, cwd: &Path) -> Result<Vec<CustomAgent>, Error> {
        self.client.call(ClientCommand::ListAgents { path: cwd.to_path_buf() }, |event| match event {
            ServerEvent::Agents(agents) => Some(Ok(agents)),
            _ => None,
        })
    }

    pub fn settings(&self) -> Result<serde_json::Value, Error> {
        self.client.call(ClientCommand::LoadSettings, |event| match event {
            ServerEvent::Settings { settings } => Some(Ok(settings.unwrap_or_else(|| serde_json::json!({})))),
            _ => None,
        })
    }

    pub fn save_settings(&self, settings: serde_json::Value) -> Result<(), Error> {
        self.client.send(ClientCommand::SaveSettings { settings })
    }

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

#[derive(Debug)]
pub struct ProjectMemory<'a> {
    sdk: &'a Sdk,
    path: PathBuf,
}

impl ProjectMemory<'_> {
    pub fn list(&self) -> Result<Vec<Memory>, Error> {
        self.answer(json!({ "op": "list" })).map(|(memories, _)| memories)
    }

    pub fn search(&self, query: &str) -> Result<Vec<(Memory, f32)>, Error> {
        let found = self.ask(json!({ "op": "search", "query": query }), "scores")?;
        let scores: Vec<(MemoryId, f32)> = parse(&found["scores"])?;
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
        self.answer(json!({ "op": "edit", "edits": edits })).map(|(memories, _)| memories)
    }

    pub fn restore(&self, id: MemoryId) -> Result<Vec<Memory>, Error> {
        self.edit(vec![MemoryEdit::Restore { id }])
    }

    pub fn forgotten(&self) -> Result<Vec<Forgotten>, Error> {
        self.answer(json!({ "op": "list" })).map(|(_, forgotten)| forgotten)
    }

    pub fn optimize(&self, model: ModelOption) -> Result<Run, Error> {
        let path = self.path.clone();
        Run::start(&self.sdk.client, |run| ClientCommand::Feature {
            feature: MEMORY.into(),
            input: json!({ "op": "optimize", "path": path, "run": run, "model": model }),
        })
    }

    pub fn undo(&self) -> Result<Vec<Memory>, Error> {
        self.answer(json!({ "op": "undo" })).map(|(memories, _)| memories)
    }

    fn answer(&self, input: Value) -> Result<(Vec<Memory>, Vec<Forgotten>), Error> {
        let answer = self.ask(input, "memories")?;
        Ok((parse(&answer["memories"])?, parse(&answer["forgotten"])?))
    }

    fn ask(&self, mut input: Value, holding: &'static str) -> Result<Value, Error> {
        input["path"] = json!(self.path);
        let path = json!(self.path);
        let command = ClientCommand::Feature { feature: MEMORY.into(), input };
        self.sdk.client.call(command, |event| match event {
            ServerEvent::Feature { feature, output } if feature == MEMORY => match output {
                Ok(answer) if answer["path"] == path && answer.get(holding).is_some() => Some(Ok(answer)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            },
            _ => None,
        })
    }
}

const MEMORY: &str = "memory";

fn parse<T: serde::de::DeserializeOwned + Default>(value: &Value) -> Result<T, Error> {
    if value.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(value.clone()).map_err(|error| Error::Backend(error.to_string()))
}
