//! Superfik domain model.
//!
//! This crate is pure: no IO, no threads, no rendering. Every other crate
//! (engines, backends, UI) speaks in these types, which keeps the boundary
//! between "what the product means" and "how it is delivered" explicit.

pub mod agents;
pub mod approval;
pub mod billing;
pub mod command;
pub mod credentials;
pub mod engines;
pub mod events;
pub mod executable;
pub mod extensions;
pub mod failure;
pub mod ids;
pub mod image;
pub mod mcp;
pub mod mcp_config;
pub mod memory;
pub mod model;
pub mod principal;
pub mod provider;
pub mod request;
pub mod skill;
pub mod usage;

pub use agents::{AgentStatus, CustomAgent};
pub use approval::{AgentAsk, AskId, AskKind, Choice, Decision, PermissionMode, Question};
pub use command::Command;
pub use engines::EngineProblem;
pub use events::{EngineEvent, RunId, ToolCall};
pub use extensions::Extensions;
pub use failure::FailureKind;
pub use ids::{Id, OrgId, SecretId, SessionId, UserId};
pub use image::{Image, MAX_IMAGE_BYTES};
pub use memory::{
    Author, Memory, MemoryBook, MemoryBrief, MemoryEdit, MemoryId, MemoryKind, MemoryTool, Provenance, RecallBudget,
};
pub use model::{Effort, ModelOption, ModelProvider};
pub use principal::{Member, Principal};
pub use request::PromptRequest;
pub use skill::Skill;
pub use usage::{
    ChoreAnswer, Compaction, ContextShare, ContextSlice, ContextUsage, HeadsUp, SessionCommand, compact_tokens,
};
