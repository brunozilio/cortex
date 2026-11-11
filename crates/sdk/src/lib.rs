//! The app's SDK: talk to the agent from your own program and extend it.
//!
//! ```no_run
//! use sdk::{Agent, Command, McpConnection, McpServer, Sdk};
//! use serde_json::json;
//!
//! # fn main() -> Result<(), sdk::Error> {
//! let sdk = Sdk::builder()
//!     .agent(Agent::new("reviewer", "Reviews a diff for bugs.").tools(["Read", "Grep"]))
//!     .command(Command::new("audit", "Audits a path.", "Audit $ARGUMENTS for security issues."))
//!     .mcp(McpServer::new("math").tool("add", "Adds a and b.", json!({"type": "object"}), |input| {
//!         Ok(json!(input["a"].as_f64().unwrap_or(0.0) + input["b"].as_f64().unwrap_or(0.0)))
//!     }))
//!     .connect(McpConnection::stdio("files", "npx").arg("-y").arg("@modelcontextprotocol/server-filesystem"))
//!     .build()?;
//! let model = sdk.models()?.into_iter().next().ok_or(sdk::Error::NoModel)?;
//! let session = sdk.session(".", model);
//! println!("{}", session.send("What does this project do?")?.text()?);
//! # Ok(()) }
//! ```
//!
//! Everything runs in this process on a [`LocalBackend`]: the agent loop,
//! its tools, memory and sessions. Commands carry the local owner's
//! `Principal`, so the same calls can reach a remote backend later.

mod account;
mod chores;
mod client;
#[cfg(unix)]
pub mod daemon;
mod extensions;
mod local_models;
mod mcp;
mod mcp_servers;
mod plan_watch;
mod run;
mod session;

use std::path::PathBuf;
use std::sync::Arc;

use backend::{Extensions, LocalBackend, LocalServices};
use domain::ModelOption;

pub use self::account::ProjectMemory;
pub use self::extensions::{Agent, Command, Skill};
pub use self::local_models::{LocalModelError, LocalModels, PullProgress, Pulled, Removed};
pub use self::mcp::{McpConnection, McpServer};
pub use self::mcp_servers::Schedules;
pub use self::run::{Run, RunControl};
pub use self::session::{Headless, Session};
use client::Client;
pub use domain::mcp_config::{McpOrigin, McpServerInfo, McpState};
pub use domain::memory::{Forgotten, Memory, MemoryEdit, MemoryId, MemoryKind, Revision};
pub use domain::provider::{ModelProvider, ProviderSummary};
pub use domain::schedule::{Scheduled, When};
pub use domain::{
    AgentAsk, AskId, AskKind, Choice, ChoreAnswer, Criteria, CriteriaSource, CustomAgent, Decision, Effort,
    EngineEvent, HeadsUp, Image, MAX_IMAGE_BYTES, ModelOption as Model, PermissionMode, Question, Rewound, SessionId,
    ToolCall, Verdict,
};
use extensions::Definitions;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Backend(String),
    #[error("the backend did not answer in time")]
    Timeout,
    #[error("the run failed: {0}")]
    Run(String),
    #[error("no model is available: connect a provider first")]
    NoModel,
    #[error("there is no conversation to compact yet")]
    NothingToCompact,
    #[error("there is no conversation to ask about yet")]
    NothingToAsk,
    #[error("could not write the SDK's agents, skills and commands: {0}")]
    Write(#[from] std::io::Error),
}

/// One embedded the app: start [`Session`]s, list models, manage
/// providers and memory.
#[derive(Debug, Clone)]
pub struct Sdk {
    client: Arc<Client>,
}

impl Sdk {
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Runs on another backend (the shared daemon, later a remote server):
    /// `connect` gets the callback the backend calls when events arrive.
    /// Agents, skills, commands and MCP servers added on a [`Builder`] are
    /// not available this way; that backend has its own.
    pub fn with_backend(
        connect: impl FnOnce(protocol::Wake) -> Result<Box<dyn protocol::Backend>, String>,
    ) -> Result<Self, Error> {
        let (wake_tx, woken) = crossbeam_channel::bounded(1);
        let wake = Arc::new(move || {
            let _ = wake_tx.try_send(());
        });
        let backend = connect(wake).map_err(Error::Backend)?;
        Ok(Self { client: Arc::new(Client::new(backend, woken)) })
    }

    /// Connects to this computer's shared daemon, starting it with `start`
    /// (which launches `serve_daemon` in the background) when none answers.
    #[cfg(unix)]
    pub fn connect_daemon(start: impl FnOnce() -> std::io::Result<()>) -> Result<Self, Error> {
        let dir = backend::daemon::DaemonDir::system().ok_or(Error::Backend("no data folder".into()))?;
        Self::with_backend(|wake| {
            backend::daemon::connect_or_start(&dir, wake, start)
                .map(|backend| Box::new(backend) as Box<dyn protocol::Backend>)
                .map_err(|error| error.to_string())
        })
    }

    /// A conversation with the agent working in `cwd`.
    pub fn session(&self, cwd: impl Into<PathBuf>, model: ModelOption) -> Session {
        let cwd = cwd.into();
        let cwd = std::path::absolute(&cwd).unwrap_or(cwd);
        Session::new(Arc::clone(&self.client), cwd, model)
    }
}

/// Sets up an [`Sdk`]: what it adds to every run, and where it keeps
/// its data.
#[derive(Default)]
pub struct Builder {
    name: Option<String>,
    data: Option<PathBuf>,
    services: Option<LocalServices>,
    definitions: Definitions,
    connections: Vec<McpConnection>,
    servers: Vec<McpServer>,
}

impl std::fmt::Debug for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builder").field("name", &self.name).field("data", &self.data).finish_non_exhaustive()
    }
}

impl Builder {
    /// Names this program's agents, skills and commands folder (`sdk` by
    /// default); two programs with different names never overwrite each
    /// other's.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Keeps sessions, memory, settings and providers under `dir` instead
    /// of the system's the app folder.
    pub fn data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data = Some(dir.into());
        self
    }

    pub fn agent(mut self, agent: Agent) -> Self {
        self.definitions.agents.push(agent);
        self
    }

    pub fn skill(mut self, skill: Skill) -> Self {
        self.definitions.skills.push(skill);
        self
    }

    pub fn command(mut self, command: Command) -> Self {
        self.definitions.commands.push(command);
        self
    }

    /// Tools written in code, served in this process.
    pub fn mcp(mut self, server: McpServer) -> Self {
        self.servers.push(server);
        self
    }

    /// An MCP server over HTTP or stdio.
    pub fn connect(mut self, connection: McpConnection) -> Self {
        self.connections.push(connection);
        self
    }

    /// Runs on these services instead of the detected ones (tests, or a
    /// custom engine).
    #[doc(hidden)]
    pub fn services(mut self, services: LocalServices) -> Self {
        self.services = Some(services);
        self
    }

    pub fn build(self) -> Result<Sdk, Error> {
        let mut services = self.services.unwrap_or_else(LocalServices::detect);
        if let Some(data) = &self.data {
            services = services.kept_in(data);
        }
        let mut extensions = Extensions::default();
        if !self.definitions.is_empty() {
            let base = self.data.clone().or_else(backend::local_data_dir).unwrap_or_else(std::env::temp_dir);
            let dir = base.join("sdk").join(self.name.as_deref().unwrap_or("sdk"));
            self.definitions.write(&dir)?;
            extensions.dirs.push(dir);
        }
        extensions.mcp = self.connections.into_iter().map(|connection| connection.0).collect();
        extensions.local = self
            .servers
            .into_iter()
            .map(|server| domain::mcp::LocalMcp::new(server.name().to_owned(), Arc::new(server)))
            .collect();
        services.extensions = extensions;
        let (wake_tx, woken) = crossbeam_channel::bounded(1);
        let wake = Arc::new(move || {
            let _ = wake_tx.try_send(());
        });
        let backend = LocalBackend::new(services, wake).map_err(|error| Error::Backend(error.to_string()))?;
        Ok(Sdk { client: Arc::new(Client::new(Box::new(backend), woken)) })
    }
}
