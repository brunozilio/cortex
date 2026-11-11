use std::path::Path;

use agent_engine::sessions::{FileSessions, records};
use domain::{ModelOption, Principal, PromptRequest, SessionId};
use serde_json::Value;

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let session = input["session"].as_str().ok_or("the trace feature needs a session")?;
    let cwd = input["cwd"].as_str().ok_or("the trace feature needs the session's folder")?;
    let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
    let store = FileSessions::new(data.join(crate::native::SESSIONS));
    traced(&store, context.principal, Path::new(cwd), &SessionId(session.into()))
}

pub fn forget(_client: u64) {}

fn traced(store: &FileSessions, owner: &Principal, cwd: &Path, id: &SessionId) -> Result<Value, String> {
    let mut request = PromptRequest::plain(cwd.into(), "", ModelOption::new("", ""));
    request.credentials.owner = Some(owner.clone());
    let records = records(store.entries(&request, id)?);
    if records.is_empty() {
        return Err("this session has nothing to trace yet".into());
    }
    serde_json::to_value(domain::trace::build(records)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
