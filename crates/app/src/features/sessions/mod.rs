use std::path::Path;

use crate::api::features::sessions::{SessionsReply, SessionsRequest};
use agent_engine::sessions::{FileSessions, Sessions, records};
use domain::catalog::Catalog;
use domain::trace::Record;
use domain::{ModelOption, PromptRequest, SessionId};

use crate::features::context::Context;

pub fn handle(context: &Context<'_>, request: SessionsRequest) -> Result<SessionsReply, String> {
    let store = context.services.sessions.as_ref();
    let change: Box<dyn Fn(&mut Catalog)> = match request {
        SessionsRequest::List => return Ok(SessionsReply::Listed(listed(context).listed().to_vec())),
        SessionsRequest::History { session, cwd } => {
            return history(store, &cwd, &session).map(SessionsReply::History);
        }
        SessionsRequest::Length { session, cwd } => return length(store, &cwd, &session).map(SessionsReply::Length),
        SessionsRequest::Rename { session, title } => Box::new(move |catalog| catalog.rename(&session, &title)),
        SessionsRequest::Archive { session, archived } => Box::new(move |catalog| catalog.archive(&session, archived)),
        SessionsRequest::Delete { session } => Box::new(move |catalog| catalog.delete(&session)),
        SessionsRequest::Seen { session } => Box::new(move |catalog| catalog.seen(&session)),
    };
    context.services.catalog.update(&mut |catalog| change(catalog));
    Ok(SessionsReply::Changed)
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
