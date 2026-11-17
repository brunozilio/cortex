mod account;
mod catalog;
mod chores;
mod client;
mod features;
mod mcp_servers;
mod memory;
mod plan_watch;
mod project;
mod run;
mod session;
#[cfg(test)]
mod testing;
mod workspace;

use std::path::PathBuf;
use std::sync::Arc;

use backend::{LocalBackend, LocalServices};
use domain::ModelOption;
use protocol::Wake;

pub use self::account::SignedIn;
pub use self::memory::ProjectMemory;
pub use self::run::{Run, RunControl};
pub use self::session::{Headless, Session, Spending};
use client::Client;
pub use domain::mcp_config::{McpOrigin, McpServerInfo, McpState};
pub use domain::memory::seed::Candidate as SeedCandidate;
pub use domain::memory::{Forgotten, Memory, MemoryEdit, MemoryId, MemoryKind, Revision};
pub use domain::provider::{ModelProvider, PortHolder, ProviderSummary};
pub use domain::schedule::{Scheduled, When};
pub use domain::{
    AgentAsk, AskId, AskKind, Choice, ChoreAnswer, Criteria, CriteriaSource, CustomAgent, Decision, Effort,
    EngineEvent, HeadsUp, Image, MAX_IMAGE_BYTES, ModelOption as Model, PermissionMode, Question, Rewound, SessionId,
    ToolCall, Verdict,
};
pub use protocol;
pub use protocol::features::flags::ListedFlag;
pub use protocol::features::plugins::{Plugin, PluginsReply, PluginsRequest};
pub use protocol::features::shell::BackgroundCommand;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Backend(String),
    #[error(transparent)]
    Feature(#[from] protocol::FeatureError),
    #[error("the backend did not answer in time")]
    Timeout,
    #[error("the run failed: {0}")]
    Run(String),
    #[error("there is no conversation to compact yet")]
    NothingToCompact,
    #[error("there is no conversation to ask about yet")]
    NothingToAsk,
}

#[derive(Debug, Clone)]
pub struct Sdk {
    client: Arc<Client>,
    services: LocalServices,
}

impl Sdk {
    pub fn builder() -> Builder {
        Builder::default()
    }

    pub fn sibling(&self) -> Result<Self, Error> {
        Self::local(self.services.clone())
    }

    fn local(services: LocalServices) -> Result<Self, Error> {
        let (wake_tx, woken) = crossbeam_channel::bounded(1);
        let wake: Wake = Arc::new(move || {
            let _ = wake_tx.try_send(());
        });
        let backend = LocalBackend::new(services.clone(), wake).map_err(|error| Error::Backend(error.to_string()))?;
        Ok(Self { client: Arc::new(Client::new(Box::new(backend), woken)), services })
    }

    pub fn session(&self, cwd: impl Into<PathBuf>, model: ModelOption) -> Session {
        let cwd = cwd.into();
        let cwd = std::path::absolute(&cwd).unwrap_or(cwd);
        Session::new(Arc::clone(&self.client), cwd, model)
    }
}

#[derive(Default)]
pub struct Builder {
    data: Option<PathBuf>,
    cwd: Option<PathBuf>,
    services: Option<LocalServices>,
}

impl std::fmt::Debug for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builder").field("data", &self.data).finish_non_exhaustive()
    }
}

impl Builder {
    pub fn data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data = Some(dir.into());
        self
    }

    pub fn cwd(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    #[doc(hidden)]
    pub fn services(mut self, services: LocalServices) -> Self {
        self.services = Some(services);
        self
    }

    pub fn build(self) -> Result<Sdk, Error> {
        let mut services = self.services.unwrap_or_else(|| LocalServices::detect_in(self.cwd.as_deref()));
        if let Some(data) = &self.data {
            services = services.kept_in(data);
        }
        Sdk::local(services)
    }
}
