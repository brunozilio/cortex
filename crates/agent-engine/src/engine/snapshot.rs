use domain::SessionId;

use super::job::Job;
use crate::prompt::Snapshot;

impl Job {
    pub(super) fn session_snapshot(&self, session: &SessionId, fresh: Snapshot) -> Snapshot {
        if let Some(kept) = self.sessions.load_snapshot(&self.request, session) {
            return kept;
        }
        if let Err(error) = self.sessions.save_snapshot(&self.request, session, &fresh) {
            tracing::warn!(%error, "the session's prompt snapshot was not kept; the next prompt takes a new one");
        }
        fresh
    }
}
