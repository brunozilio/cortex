//! Messages a backend sends to its client.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use superfik_core::Principal;
use superfik_core::access::Colleague;
use superfik_core::autofill::{Candidate, LoginFill};
use superfik_core::{
    BrowserId, Connection, CustomAgent, EngineEvent, EngineInfo, GitHubRepository, Memory, MemoryId, ModelOption,
    PluginId, PluginSummary, PullRequest, PullRequestDetail, PullRequestDraft, Repository, ReviewCommentDraft, RunId,
    SessionRecord, Skill, WorkspaceSnapshot,
};

use crate::carried::{LinkState, MemberUsage, SharedSession, Viewer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerEvent {
    Welcome {
        version: u32,
        principal: Principal,
    },
    Members(Vec<Colleague>),
    FirstAccessCode {
        user: superfik_core::UserId,
        name: String,
        code: String,
    },
    ServerSettings(crate::carried::ServerSettings),
    Run {
        run: RunId,
        event: EngineEvent,
    },
    Models(Vec<ModelOption>),
    Engines(Vec<EngineInfo>),
    Workspace(WorkspaceSnapshot),
    Repository(Repository),
    GitHubRepositories {
        signed_in: bool,
        repositories: Vec<GitHubRepository>,
    },
    Settings {
        settings: Option<serde_json::Value>,
    },
    GitHubCode {
        code: String,
        url: String,
    },
    GitHubSignedIn {
        login: Option<String>,
    },
    Uploaded {
        from: PathBuf,
        path: PathBuf,
    },
    RepositoryLocated {
        slug: String,
        path: PathBuf,
    },
    PullRequests(Vec<PullRequest>),
    PullRequestCreated(PullRequest),
    PullRequestProposed(PullRequestDraft),
    PullRequestLoaded(Box<PullRequestDetail>),
    ReviewDrafted {
        number: u64,
        summary: String,
        comments: Vec<ReviewCommentDraft>,
    },
    ReviewSubmitted {
        number: u64,
    },
    ThreadResolved {
        thread: String,
        resolved: bool,
    },
    Skills(Vec<Skill>),
    Agents(Vec<CustomAgent>),
    Memory {
        path: PathBuf,
        memories: Vec<Memory>,
        learned: Vec<MemoryId>,
    },
    Notes {
        path: PathBuf,
        notes: Vec<superfik_core::note::Note>,
        #[serde(default)]
        by: Option<RunId>,
        #[serde(default)]
        created: Vec<superfik_core::note::NoteId>,
        #[serde(default)]
        refused: Vec<superfik_core::note::NoteId>,
    },
    Routines {
        path: PathBuf,
        book: superfik_core::routine::RoutineBook,
    },
    RoutineWebhook {
        path: PathBuf,
        routine: superfik_core::routine::RoutineId,
        hook: superfik_core::routine::WebhookId,
        token: superfik_core::routine::WebhookToken,
    },
    Channel {
        connection: superfik_core::ConnectionId,
        status: superfik_core::channel::LinkStatus,
        drafts: Vec<superfik_core::channel::Draft>,
    },
    MemorySearch {
        path: PathBuf,
        query: String,
        scores: Vec<(MemoryId, f32)>,
    },
    Sessions {
        path: PathBuf,
        sessions: Vec<SessionRecord>,
    },
    SharedSessions {
        project: superfik_core::project::ProjectRef,
        sessions: Vec<SharedSession>,
    },
    SharedMemory {
        project: superfik_core::project::ProjectRef,
        memories: Vec<superfik_core::memory::shared::SharedMemory>,
    },
    Policy(superfik_core::policy::Policy),
    Spent {
        month: String,
        usd: f64,
    },
    Usage(Vec<MemberUsage>),
    Comments {
        session: superfik_core::ConversationId,
        comments: Vec<superfik_core::comments::Comment>,
    },
    Teams(Vec<superfik_core::sharing::Team>),
    Presence(Vec<Viewer>),
    Library(Vec<superfik_core::library::LibraryItem>),
    LibraryItemRead(superfik_core::library::LibraryItem),
    Delegated(Vec<superfik_core::delegation::DelegatedAsk>),
    DelegatedAnswered {
        run: RunId,
        ask: superfik_core::AskId,
        decision: superfik_core::Decision,
        by: String,
    },
    Plugins {
        plugins: Vec<PluginSummary>,
        connections: Vec<Connection>,
    },
    AuthorizeAt {
        plugin: PluginId,
        url: String,
    },
    PluginFailed {
        plugin: Option<PluginId>,
        message: String,
    },
    Providers(crate::ProviderEvent),
    LoginCandidates {
        browser: BrowserId,
        url: String,
        candidates: Vec<Candidate>,
    },
    LoginSecret {
        browser: BrowserId,
        url: String,
        fill: LoginFill,
    },
    LoginFailed {
        browser: BrowserId,
        message: String,
    },
    LoginSaved {
        browser: BrowserId,
        account: String,
    },
    PasskeyCandidates {
        browser: BrowserId,
        rp_id: String,
        passkeys: Vec<superfik_core::passkey::StoredPasskey>,
    },
    PasskeySigned {
        browser: BrowserId,
        assertion: superfik_core::passkey::Assertion,
    },
    PasskeyCreated {
        browser: BrowserId,
        attestation: superfik_core::passkey::Attestation,
    },
    PasskeyFailed {
        browser: BrowserId,
        message: String,
    },
    Page {
        browser: BrowserId,
        report: superfik_core::page::PageReport,
    },
    PageFrame(superfik_core::page::PageFrame),
    Error {
        message: String,
    },
    Link(LinkState),
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
