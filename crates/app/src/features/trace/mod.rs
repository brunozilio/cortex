use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::features::trace::{TraceReply, TraceRequest};
use agent_engine::sessions::{FileSessions, records};
use domain::SessionId;
use domain::trace::insights::{Report, SessionRuns, insights, recent};
use domain::trace::runs::{Run, runs, trim, without_messages};
use serde_json::Value;

use crate::features::context::Context;

pub mod export;

const CURRENT_TEXT: usize = 20_000;
const OTHER_TEXT: usize = 2_000;

pub fn handle(context: &Context<'_>, request: TraceRequest) -> Result<TraceReply, String> {
    let store = context.services.sessions.as_ref();
    let report = match request {
        TraceRequest::EverySession => {
            let catalog = context.services.catalog.read();
            let listed: Vec<(SessionId, PathBuf)> =
                catalog.listed().iter().map(|entry| (entry.session.clone(), entry.cwd.clone())).collect();
            everything(store, &listed, now_ms())
        }
        TraceRequest::Session { session, cwd } => {
            let others = crate::features::sessions::recent(context, &cwd);
            reported(store, &cwd, &session, &others, now_ms())?
        }
        TraceRequest::Follow { session, cwd } => {
            return followed(store, &cwd, &session, now_ms()).map(TraceReply::Followed);
        }
    };
    serde_json::to_value(report).map(TraceReply::Report).map_err(|e| e.to_string())
}

pub fn forget(_client: u64) {}

fn followed(store: &FileSessions, cwd: &Path, id: &SessionId, now_ms: u64) -> Result<Value, String> {
    let runs = traced(store, cwd, id)?;
    let recent_runs: Vec<&Run> = runs.iter().filter(|run| recent(run, now_ms)).collect();
    let insights = serde_json::to_value(insights(&recent_runs, now_ms)).map_err(|e| e.to_string())?;
    let mut update = serde_json::to_value(SessionRuns { id: id.0.clone(), runs }).map_err(|e| e.to_string())?;
    update["insights"] = insights;
    Ok(update)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

fn everything(store: &FileSessions, listed: &[(SessionId, PathBuf)], now_ms: u64) -> Report {
    let mut sessions: Vec<SessionRuns> = listed
        .iter()
        .filter_map(|(id, cwd)| {
            let mut runs = traced(store, cwd, id).ok()?;
            runs.retain(|run| recent(run, now_ms));
            trim(&mut runs, OTHER_TEXT);
            without_messages(&mut runs);
            (!runs.is_empty()).then(|| SessionRuns { id: id.0.clone(), runs })
        })
        .collect();
    let latest = |session: &SessionRuns| session.runs.iter().map(|run| run.started_at_ms).max().unwrap_or_default();
    sessions.sort_by_key(|session| std::cmp::Reverse(latest(session)));
    let all: Vec<&Run> = sessions.iter().flat_map(|session| &session.runs).collect();
    let insights = insights(&all, now_ms);
    let current = sessions.first().map(|session| session.id.clone()).unwrap_or_default();
    Report { project: String::new(), current, generated_at_ms: now_ms, sessions, insights }
}

pub(crate) fn traced(store: &FileSessions, cwd: &Path, id: &SessionId) -> Result<Vec<Run>, String> {
    let records = records(store.entries(cwd, id)?);
    if records.is_empty() {
        return Err("this session has nothing to trace yet".into());
    }
    Ok(runs(&id.0, &domain::trace::build(records)))
}

fn reported(
    store: &FileSessions,
    cwd: &Path,
    current: &SessionId,
    others: &[SessionId],
    now_ms: u64,
) -> Result<Report, String> {
    let mut mine = traced(store, cwd, current)?;
    trim(&mut mine, CURRENT_TEXT);
    let mut sessions = vec![SessionRuns { id: current.0.clone(), runs: mine }];
    for id in others.iter().filter(|id| *id != current) {
        let Ok(mut runs) = traced(store, cwd, id) else { continue };
        runs.retain(|run| recent(run, now_ms));
        trim(&mut runs, OTHER_TEXT);
        without_messages(&mut runs);
        if !runs.is_empty() {
            sessions.push(SessionRuns { id: id.0.clone(), runs });
        }
    }
    let all: Vec<&Run> = sessions.iter().flat_map(|session| &session.runs).collect();
    let insights = insights(&all, now_ms);
    Ok(Report {
        project: cwd.display().to_string(),
        current: current.0.clone(),
        generated_at_ms: now_ms,
        sessions,
        insights,
    })
}

#[cfg(test)]
mod tests;
