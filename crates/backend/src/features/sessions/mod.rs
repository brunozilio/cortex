use std::path::Path;

use agent_engine::sessions::{FileSessions, Sessions, records};
use domain::catalog::Catalog;
use domain::trace::Record;
use domain::{ModelOption, PromptRequest, SessionId};
use serde_json::{Value, json};

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let op = input["op"].as_str().ok_or("the sessions feature needs an op")?;
    if op == "list" {
        return Ok(json!({ "sessions": listed(context).listed() }));
    }
    let session = SessionId(input["session"].as_str().ok_or("this op needs a session")?.to_owned());
    if op == "history" || op == "length" {
        let cwd = Path::new(input["cwd"].as_str().ok_or("this op needs the session's folder")?);
        let store = context.services.sessions.as_ref();
        if op == "length" {
            return Ok(json!({ "length": length(store, cwd, &session)? }));
        }
        return Ok(json!({ "records": history(store, cwd, &session)? }));
    }
    let change: Box<dyn Fn(&mut Catalog)> = match op {
        "rename" => {
            let title = input["title"].as_str().ok_or("rename needs a title")?.to_owned();
            Box::new(move |catalog| catalog.rename(&session, &title))
        }
        "archive" => {
            let archived = input["archived"].as_bool().unwrap_or(true);
            Box::new(move |catalog| catalog.archive(&session, archived))
        }
        "delete" => Box::new(move |catalog| catalog.delete(&session)),
        "seen" => Box::new(move |catalog| catalog.seen(&session)),
        other => return Err(format!("the sessions feature has no op {other}")),
    };
    context.services.catalog.update(&mut |catalog| change(catalog));
    Ok(json!({}))
}

pub fn forget(_client: u64) {}

fn kept_in(cwd: &Path) -> PromptRequest {
    PromptRequest::plain(cwd.into(), "", ModelOption::new("", ""))
}

fn length(store: &FileSessions, cwd: &Path, id: &SessionId) -> Result<u64, String> {
    store.length(&kept_in(cwd), id)
}

fn history(store: &FileSessions, cwd: &Path, id: &SessionId) -> Result<Vec<Record>, String> {
    let told = records(store.entries(&kept_in(cwd), id)?);
    Ok(told
        .into_iter()
        .filter(|record| matches!(record, Record::Prompted { .. } | Record::Shown { .. } | Record::Event { .. }))
        .collect())
}

#[cfg(test)]
mod tests;

fn listed(context: &Context<'_>) -> Catalog {
    let catalog = context.services.catalog.read();
    if !catalog.listed().is_empty() {
        return catalog;
    }
    let Some(owner_settings) = context.services.settings.load() else { return catalog };
    context.services.catalog.update(&mut |catalog| catalog.import(&owner_settings));
    context.services.catalog.read()
}
