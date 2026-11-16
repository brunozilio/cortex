use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_engine::sessions::{FileSessions, records};
use domain::trace::insights::{Report, SessionRuns, insights, recent};
use domain::trace::runs::{Run, runs, trim, without_messages};
use domain::{ModelOption, Principal, PromptRequest, SessionId};
use serde_json::Value;

use crate::feature::Context;

const CURRENT_TEXT: usize = 20_000;
const OTHER_TEXT: usize = 2_000;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    if input["all"] == true {
        let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        let store = FileSessions::new(data.join(crate::native::SESSIONS));
        let catalog = context.services.catalog.read(context.principal);
        let listed: Vec<(SessionId, PathBuf)> = catalog
            .listed()
            .iter()
            .filter(|entry| !entry.archived)
            .map(|entry| (entry.session.clone(), entry.cwd.clone()))
            .collect();
        let report = everything(&store, context.principal, &listed, now_ms());
        return serde_json::to_value(report).map_err(|e| e.to_string());
    }
    let session = input["session"].as_str().ok_or("the trace feature needs a session")?;
    let cwd = input["cwd"].as_str().ok_or("the trace feature needs the session's folder")?;
    let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
    let store = FileSessions::new(data.join(crate::native::SESSIONS));
    if input["only"] == "current" {
        let current = SessionId(session.into());
        return followed(&store, context.principal, Path::new(cwd), &current, now_ms());
    }
    let owner_settings = context.services.settings.load(context.principal).unwrap_or_default();
    let others = crate::settings::recent_sessions(&owner_settings, Path::new(cwd));
    let report = reported(&store, context.principal, Path::new(cwd), &SessionId(session.into()), &others, now_ms())?;
    serde_json::to_value(report).map_err(|e| e.to_string())
}

pub fn forget(_client: u64) {}

fn followed(store: &FileSessions, owner: &Principal, cwd: &Path, id: &SessionId, now_ms: u64) -> Result<Value, String> {
    let runs = traced(store, owner, cwd, id)?;
    let recent_runs: Vec<&Run> = runs.iter().filter(|run| recent(run, now_ms)).collect();
    let insights = serde_json::to_value(insights(&recent_runs, now_ms)).map_err(|e| e.to_string())?;
    let mut update = serde_json::to_value(SessionRuns { id: id.0.clone(), runs }).map_err(|e| e.to_string())?;
    update["insights"] = insights;
    Ok(update)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

fn everything(store: &FileSessions, owner: &Principal, listed: &[(SessionId, PathBuf)], now_ms: u64) -> Report {
    let mut sessions: Vec<SessionRuns> = listed
        .iter()
        .filter_map(|(id, cwd)| {
            let mut runs = traced(store, owner, cwd, id).ok()?;
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

fn traced(store: &FileSessions, owner: &Principal, cwd: &Path, id: &SessionId) -> Result<Vec<Run>, String> {
    let mut request = PromptRequest::plain(cwd.into(), "", ModelOption::new("", ""));
    request.credentials.owner = Some(owner.clone());
    let records = records(store.entries(&request, id)?);
    if records.is_empty() {
        return Err("this session has nothing to trace yet".into());
    }
    Ok(runs(&id.0, &domain::trace::build(records)))
}

fn reported(
    store: &FileSessions,
    owner: &Principal,
    cwd: &Path,
    current: &SessionId,
    others: &[SessionId],
    now_ms: u64,
) -> Result<Report, String> {
    let mut mine = traced(store, owner, cwd, current)?;
    trim(&mut mine, CURRENT_TEXT);
    let mut sessions = vec![SessionRuns { id: current.0.clone(), runs: mine }];
    for id in others.iter().filter(|id| *id != current) {
        let Ok(mut runs) = traced(store, owner, cwd, id) else { continue };
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
