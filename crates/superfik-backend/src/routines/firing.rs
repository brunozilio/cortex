//! One firing: the request it sends and what its sink keeps of the run.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use superfik_workspace::Worktrees;

use superfik_core::routine::{Cause, Ending, MAX_REPORT_CHARS, Routine};
use superfik_core::{EngineEvent, MemoryBrief, ModelOption, Principal, PromptRequest, RunId};
use superfik_protocol::ServerEvent;

use super::Routines;
use crate::memory::project_key;

#[derive(Debug, Clone, PartialEq)]
pub struct Firing {
    pub owner: Principal,
    pub project: PathBuf,
    pub routine: Routine,
}

pub fn place(firing: &Firing, root: &Path, now: i64) -> Option<(String, PathBuf)> {
    let routine = &firing.routine;
    routine.works_apart().then(|| {
        let branch = routine.branch(now);
        let path = root.join(project_key(&firing.project)).join(branch.replace('/', "-"));
        (branch, path)
    })
}

pub fn request(
    firing: &Firing,
    models: &[ModelOption],
    branch: Option<&str>,
    cause: Cause<'_>,
) -> Option<PromptRequest> {
    let routine = &firing.routine;
    let named = |id: &String| {
        let provider = superfik_core::ModelProvider::ACCOUNTS
            .into_iter()
            .find(|p| id.strip_prefix(p.plugin_id()).is_some_and(|rest| rest.starts_with(':')));
        provider.map(|provider| ModelOption::new(superfik_core::EngineKind::Native, id, id).served_by(provider))
    };
    let model = match &routine.model {
        Some(id) => models.iter().find(|m| &m.id == id).cloned().or_else(|| named(id)),
        None => None,
    }
    .or_else(|| models.first().cloned())?;
    Some(PromptRequest {
        permission: routine.autonomy.mode(),
        memory: Some(MemoryBrief::new(Some(format!("routine {}", routine.name)))),
        connections: routine.connections.clone(),
        ..PromptRequest::plain(firing.project.clone(), routine.prompt(branch, cause), model)
    })
}

pub struct Trace {
    routines: Routines,
    owner: Principal,
    project: PathBuf,
    workdir: Option<PathBuf>,
    cleanup: Option<(Arc<dyn Worktrees>, String)>,
    budget: Option<(Arc<dyn crate::Budget>, String)>,
    reply: Mutex<String>,
    metered: Mutex<Option<Option<f64>>>,
}

impl Trace {
    pub fn new(routines: Routines, firing: &Firing, workdir: Option<PathBuf>) -> Self {
        let (owner, project) = (firing.owner.clone(), firing.project.clone());
        Self {
            routines,
            owner,
            project,
            workdir,
            cleanup: None,
            budget: None,
            reply: Mutex::default(),
            metered: Mutex::default(),
        }
    }

    pub fn spending(self, budget: (Arc<dyn crate::Budget>, String)) -> Self {
        Self { budget: Some(budget), ..self }
    }

    pub fn cleaning(self, worktrees: Arc<dyn Worktrees>, branch: String) -> Self {
        Self { cleanup: Some((worktrees, branch)), ..self }
    }

    fn clean(&self) -> bool {
        let (Some((worktrees, branch)), Some(path)) = (&self.cleanup, &self.workdir) else { return false };
        match worktrees.remove_if_untouched(&self.project, path, branch) {
            Ok(removed) => removed,
            Err(error) => {
                tracing::warn!(%error, "could not tidy a routine's worktree");
                false
            }
        }
    }

    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }

    pub fn observe(&self, run: RunId, event: &EngineEvent, now: i64) -> Option<ServerEvent> {
        if let EngineEvent::Usage(usage) = event {
            let mut metered = self.metered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            *metered = Some(metered.unwrap_or(Some(0.0)).zip(usage.estimated_cost_usd).map(|(a, b)| a + b));
            return None;
        }
        let mut reply = self.reply.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (ending, cost) = match event {
            EngineEvent::TextDelta(text) => {
                reply.push_str(text);
                keep_tail(&mut reply);
                return None;
            }
            EngineEvent::Completed { cost_usd, .. } => (Ending::Completed, cost_usd.unwrap_or(0.0)),
            EngineEvent::Failed(message) => (Ending::Failed(message.clone()), 0.0),
            EngineEvent::Cancelled => (Ending::Cancelled, 0.0),
            _ => return None,
        };
        let metered = *self.metered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let cost = metered.unwrap_or(Some(cost)).unwrap_or(0.0);
        let text = reply.trim();
        if let Some((budget, model)) =
            self.budget.as_ref().filter(|(budget, _)| cost > 0.0 && (metered.is_none() || !budget.authoritative()))
        {
            budget.spend(&self.owner, model, cost);
        }
        let removed = self.clean();
        self.routines.change(&self.owner, &self.project, |book| {
            let finished = book.finish(run, ending, text, cost, now);
            if finished
                && removed
                && let Some(report) = book.reports.iter_mut().find(|r| r.run == run)
            {
                report.branch = None;
            }
            finished
        })
    }
}

pub(super) fn keep_tail(reply: &mut String) {
    let count = reply.chars().count();
    if count > MAX_REPORT_CHARS {
        let cut = reply.char_indices().nth(count - MAX_REPORT_CHARS).map_or(0, |(at, _)| at);
        reply.drain(..cut);
    }
}
