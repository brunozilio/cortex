use std::path::Path;

use crate::api::features::sessions::{SessionsReply, SessionsRequest};
use agent_engine::sessions::{FileSessions, Sessions, records};
use domain::SessionId;
use domain::catalog::Catalog;
use domain::trace::Record;

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
        SessionsRequest::Delete { session } => Box::new(move |catalog| catalog.delete(&session)),
        SessionsRequest::Seen { session } => Box::new(move |catalog| catalog.seen(&session)),
    };
    context.services.catalog.update(&mut |catalog| change(catalog));
    Ok(SessionsReply::Changed)
}

pub fn forget(_client: u64) {}

fn length(store: &FileSessions, cwd: &Path, id: &SessionId) -> Result<u64, String> {
    store.length(cwd, id)
}

fn history(store: &FileSessions, cwd: &Path, id: &SessionId) -> Result<Vec<Record>, String> {
    let told = records(store.entries(cwd, id)?);
    Ok(told
        .into_iter()
        .filter(|record| matches!(record, Record::Prompted { .. } | Record::Shown { .. } | Record::Event { .. }))
        .collect())
}

#[cfg(test)]
mod tests;

pub(crate) fn recent(context: &Context<'_>, cwd: &Path) -> Vec<SessionId> {
    import_what_the_settings_listed(context);
    context.services.catalog.recent(cwd, crate::catalog::RECENT)
}

fn listed(context: &Context<'_>) -> Catalog {
    import_what_the_settings_listed(context);
    context.services.catalog.read()
}

fn import_what_the_settings_listed(context: &Context<'_>) {
    if !context.services.catalog.is_empty() {
        return;
    }
    let Some(owner_settings) = context.services.settings.load() else { return };
    context.services.catalog.update(&mut |catalog| catalog.import(&owner_settings));
}
