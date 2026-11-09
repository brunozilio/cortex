//! Presence: which session each connection has on screen. A change, or a
//! connection leaving, goes to everyone as the whole list. Kept in memory.

use std::collections::BTreeMap;
use std::sync::Mutex;

use superfik_core::{ConversationId, UserId};
use superfik_protocol::Viewer;

use crate::ConnectionId;

#[derive(Debug, Default)]
pub struct Presence {
    viewing: Mutex<BTreeMap<ConnectionId, Viewer>>,
}

impl Presence {
    fn viewing(&self) -> std::sync::MutexGuard<'_, BTreeMap<ConnectionId, Viewer>> {
        self.viewing.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set(
        &self,
        connection: ConnectionId,
        user: UserId,
        name: String,
        session: Option<ConversationId>,
    ) -> Option<Vec<Viewer>> {
        let mut viewing = self.viewing();
        let before = viewing.get(&connection).map(|viewer| viewer.session);
        if before == session {
            return None;
        }
        match session {
            Some(session) => viewing.insert(connection, Viewer { user, name, session }),
            None => viewing.remove(&connection),
        };
        Some(listed(&viewing))
    }

    pub fn leave(&self, connection: ConnectionId) -> Option<Vec<Viewer>> {
        let mut viewing = self.viewing();
        viewing.remove(&connection).map(|_| listed(&viewing))
    }
}

fn listed(viewing: &BTreeMap<ConnectionId, Viewer>) -> Vec<Viewer> {
    let mut list: Vec<Viewer> = Vec::new();
    for viewer in viewing.values() {
        if !list.iter().any(|v| v.user == viewer.user && v.session == viewer.session) {
            list.push(viewer.clone());
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hub;

    #[test]
    fn viewers_are_listed_once_and_leave_with_their_connection() {
        let mut hub = Hub::default();
        let (laptop, desktop) =
            (hub.join(superfik_core::Principal::LocalOwner), hub.join(superfik_core::Principal::LocalOwner));
        let presence = Presence::default();
        let (ana, session) = (UserId::new(), ConversationId::new());
        assert_eq!(presence.set(laptop, ana, "Ana".into(), Some(session)).map(|l| l.len()), Some(1));
        assert_eq!(presence.set(laptop, ana, "Ana".into(), Some(session)), None, "no change");
        assert_eq!(presence.set(desktop, ana, "Ana".into(), Some(session)).map(|l| l.len()), Some(1), "once");
        assert_eq!(presence.leave(laptop).map(|l| l.len()), Some(1));
        assert_eq!(presence.set(desktop, ana, "Ana".into(), None), Some(vec![]));
        assert_eq!(presence.leave(desktop), None);
    }
}
