//! Superfik domain model.
//!
//! This crate is pure: no IO, no threads, no rendering. Every other crate
//! (engines, backends, UI) speaks in these types, which keeps the boundary
//! between "what the product means" and "how it is delivered" explicit.

pub mod access;
pub mod agents;
pub mod approval;
pub mod autofill;
pub mod billing;
pub mod block;
pub mod browser;
pub mod browser_identity;
pub mod browser_profiles;
pub mod channel;
pub mod command;
pub mod comments;
pub mod context;
pub mod conversation;
pub mod credentials;
pub mod days;
pub mod delegation;
pub mod diff;
pub mod engines;
pub mod events;
pub mod executable;
pub mod failure;
pub mod gateway;
pub mod handoff;
pub mod history;
pub mod ids;
pub mod job;
pub mod library;
pub mod memory;
pub mod model;
pub mod note;
pub mod output_style;
pub mod page;
pub mod passkey;
pub mod plugins;
pub mod policy;
pub mod principal;
pub mod project;
pub mod provider;
pub mod repository;
pub mod request;
pub mod review;
pub mod routine;
pub mod sharing;
pub mod spoken;
pub mod steps;
pub mod usage;
pub mod vault;
pub mod visual;

pub use agents::{AgentRole, AgentStatus, CustomAgent, SubAgent};
pub use approval::{AgentAsk, AskId, AskKind, Choice, Decision, PermissionMode, Question};
pub use block::Block;
pub use browser::{BrowseId, BrowserAnswer, BrowserOp, BrowserTool, Locator};
pub use command::Command;
pub use context::{Reference, ResearchItem, ResearchKind, Skill, SkillIcon, WorkspaceSnapshot};
pub use conversation::{Activity, Turn, TurnResponse, UserMessage, title_from_prompt};
pub use days::DayGroup;
pub use diff::{DiffLine, FileDiff, FileStatus, Hunk, LineKind};
pub use engines::{Availability, EngineInfo, EngineProblem};
pub use events::{EngineEvent, RunId, ToolCall};
pub use failure::FailureKind;
pub use gateway::GatewayAccess;
pub use handoff::{Handoff, HandoffKind};
pub use history::SessionRecord;
pub use ids::{
    AgentId, BrowserId, ConversationId, Id, OrgId, ProjectId, SessionId, TeamId, TurnId, UserId, WorkspaceId,
};
pub use memory::{
    Author, Memory, MemoryBook, MemoryBrief, MemoryEdit, MemoryId, MemoryKind, MemoryTool, Provenance, RecallBudget,
};
pub use model::{Effort, EngineKind, ModelOption, ModelProvider};
pub use output_style::OutputStyle;
pub use plugins::{
    Accounts, AuthKind, Connection, ConnectionId, PluginBrief, PluginId, PluginOrigin, PluginSummary, SecretField,
};
pub use principal::{Member, Principal};
pub use repository::{
    BranchRef, GitHubRepository, PullRequest, PullRequestDraft, PullRequestState, Repository, ReviewDecision,
};
pub use request::PromptRequest;
pub use review::{
    CommentOrigin, CommentSide, CommentState, PullRequestDetail, ReviewComment, ReviewCommentDraft, ReviewVerdict,
};
pub use steps::{Step, StepKind};
pub use usage::{
    ChoreAnswer, Compaction, ContextShare, ContextSlice, ContextUsage, HeadsUp, SessionCommand, compact_tokens,
};
pub use vault::{SecretUse, VaultKind, VaultTool};
