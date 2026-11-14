use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_engine::sessions::{FileSessions, records};
use domain::trace::insights::{Report, SessionRuns, insights, recent};
use domain::trace::runs::{Run, runs, trim};
use domain::{ModelOption, Principal, PromptRequest, SessionId};
use serde_json::Value;

use crate::feature::Context;

const CURRENT_TEXT: usize = 20_000;
const OTHER_TEXT: usize = 2_000;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let session = input["session"].as_str().ok_or("the trace feature needs a session")?;
    let cwd = input["cwd"].as_str().ok_or("the trace feature needs the session's folder")?;
    let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
    let store = FileSessions::new(data.join(crate::native::SESSIONS));
    let owner_settings = context.services.settings.load(context.principal).unwrap_or_default();
    let others = crate::settings::recent_sessions(&owner_settings, Path::new(cwd));
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
    let report = reported(&store, context.principal, Path::new(cwd), &SessionId(session.into()), &others, now)?;
    serde_json::to_value(report).map_err(|e| e.to_string())
}

pub fn forget(_client: u64) {}

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
