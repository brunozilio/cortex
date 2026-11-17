mod cache;
mod events;
mod sources;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use domain::{PromptRequest, SessionId};
use platform::sync::Locked;

use self::cache::{FromDisk, Load, Loaded, SessionCache};
use crate::brief::Brief;
use crate::mcp::{McpPool, McpServer, McpSources};
use crate::model::http::Http;
use crate::tools::Workspace;
use crate::tools::host_judge::HostJudge;
use crate::tools::script_tool::{self, ScriptTool};

pub(super) const LIVE_SESSIONS: usize = 8;

pub(super) struct LiveSessions {
    capacity: usize,
    recent: Mutex<Vec<Arc<LiveSession>>>,
}

impl Default for LiveSessions {
    fn default() -> Self {
        Self::holding(LIVE_SESSIONS)
    }
}

impl LiveSessions {
    pub(super) fn holding(capacity: usize) -> Self {
        Self { capacity, recent: Mutex::default() }
    }

    pub(super) fn enter(&self, id: &SessionId) -> Arc<LiveSession> {
        let (session, evicted) = {
            let mut recent = self.recent.locked();
            let session = match recent.iter().position(|live| live.id == *id) {
                Some(at) => recent.remove(at),
                None => Arc::new(LiveSession::new(id.clone())),
            };
            recent.push(Arc::clone(&session));
            (session, self.evicted(&mut recent))
        };
        if let Some(evicted) = evicted {
            let ending = std::thread::Builder::new().name("agent-engine-session-end".into());
            let _ = ending.spawn(move || drop(evicted));
        }
        session
    }

    fn evicted(&self, recent: &mut Vec<Arc<LiveSession>>) -> Option<Arc<LiveSession>> {
        if recent.len() <= self.capacity {
            return None;
        }
        let newest = recent.len() - 1;
        let idle = recent.iter().take(newest).position(|live| Arc::strong_count(live) == 1)?;
        Some(recent.remove(idle))
    }

    #[cfg(test)]
    fn ids(&self) -> Vec<String> {
        self.recent.locked().iter().map(|live| live.id.0.clone()).collect()
    }
}

pub(super) struct LiveSession {
    id: SessionId,
    cache: SessionCache,
    pool: McpPool,
    started: AtomicBool,
    ending: Mutex<Option<Ending>>,
}

struct Ending {
    workspace: Workspace,
    project: PathBuf,
}

impl LiveSession {
    fn new(id: SessionId) -> Self {
        Self {
            id,
            cache: SessionCache::default(),
            pool: McpPool::default(),
            started: AtomicBool::new(false),
            ending: Mutex::default(),
        }
    }

    fn begins_run(&self, workspace: &Workspace, project: PathBuf) -> bool {
        let mut kept = workspace.fork(Arc::default());
        kept.services = Default::default();
        *self.ending.locked() = Some(Ending { workspace: kept, project });
        !self.started.swap(true, Ordering::SeqCst)
    }

    fn end(&mut self) {
        let ending = self.ending.get_mut().unwrap_or_else(PoisonError::into_inner).take();
        let Some(Ending { mut workspace, project }) = ending.filter(|_| *self.started.get_mut()) else { return };
        if !workspace.cwd.is_dir() {
            workspace.cwd = project;
        }
        let ended = serde_json::json!({"session_id": self.id});
        if let Err(error) = workspace.hooks.run(crate::hooks::HookEvent::SessionEnd, "", ended, &workspace) {
            tracing::warn!(%error, "a SessionEnd hook failed");
        }
    }
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        self.end();
        self.pool.close();
    }
}

pub(super) struct Live {
    sessions: Option<Arc<LiveSessions>>,
    entered: Option<Arc<LiveSession>>,
    scripts: Option<Vec<ScriptTool>>,
}

impl Live {
    pub(super) fn of(sessions: &Arc<LiveSessions>) -> Self {
        Self { sessions: Some(Arc::clone(sessions)), entered: None, scripts: None }
    }

    pub(super) fn enter(&mut self, id: &SessionId) {
        self.entered = self.sessions.take().map(|sessions| sessions.enter(id));
    }

    pub(super) fn leave(&mut self) {
        self.sessions = None;
        self.entered = None;
    }

    pub(super) fn brief(&mut self, request: &PromptRequest) -> Result<Brief, String> {
        let loaded = match &self.entered {
            Some(session) => session.cache.loaded(request, &FromDisk)?,
            None => Arc::new(FromDisk.load(request)?),
        };
        let Loaded { brief, scripts } = Loaded::clone(&loaded);
        self.scripts = Some(scripts);
        Ok(brief)
    }

    pub(super) fn scripts(&mut self, brief: &Brief, cwd: &std::path::Path) -> Vec<ScriptTool> {
        self.scripts.take().unwrap_or_else(|| script_tool::load(&brief.gate(cwd)))
    }

    pub(super) fn connect(
        &self,
        request: &PromptRequest,
        sources: &McpSources,
        http: &Arc<dyn Http>,
        stop: &platform::Stop,
        judge: &Arc<HostJudge>,
    ) -> Vec<Arc<McpServer>> {
        match &self.entered {
            Some(session) => session.pool.connect(request, sources, http, stop, judge),
            None => crate::mcp::connect_judged(request, sources, http, stop, judge),
        }
    }

    pub(super) fn begins_session(&self, workspace: &Workspace, project: PathBuf) -> bool {
        self.entered.as_ref().is_none_or(|session| session.begins_run(workspace, project))
    }
}
