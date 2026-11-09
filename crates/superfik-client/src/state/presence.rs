//! Presence in an organization: the session this member has on screen goes
//! to the server, and the timeline names who else has each one open.

use superfik_core::ConversationId;
use superfik_protocol::{ClientCommand, Viewer};

use super::{Effect, Session, TimelineGroup};

impl Session {
    pub(super) fn presence_arrived(&mut self, viewers: Vec<Viewer>) {
        self.presence = viewers;
    }

    pub(super) fn on_screen(&self) -> Option<ConversationId> {
        let c = self.conversation();
        (c.started_unix.is_some() || c.shared_by.is_some()).then_some(c.id)
    }

    pub(super) fn report_viewing(&self, before: Option<ConversationId>) -> Vec<Effect> {
        let session = self.on_screen();
        if !self.organization.joined() || session == before {
            return Vec::new();
        }
        let mut effects = vec![Effect::Command(Box::new(ClientCommand::Viewing { session }))];
        effects.extend(session.map(|session| self.load_comments(session)).unwrap_or_default());
        effects
    }

    pub fn others_here(&self, session: ConversationId) -> Vec<&str> {
        let me = self.organization.me.map(|me| me.user);
        self.presence
            .iter()
            .filter(|viewer| viewer.session == session && Some(viewer.user) != me)
            .map(|viewer| viewer.name.as_str())
            .collect()
    }

    pub(super) fn mark_presence(&self, groups: &mut [TimelineGroup]) {
        for item in groups.iter_mut().flat_map(|group| group.items.iter_mut()) {
            let here = self.others_here(item.id);
            if !here.is_empty() {
                let here = self.strings().t(superfik_i18n::Text::Here(&here.join(", ")));
                item.title = format!("{} · {here}", item.title);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Member, OrgId, Principal, UserId};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey};

    fn joined() -> (Session, Member) {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some("/repo".into());
        (session, me)
    }

    #[test]
    fn switching_sessions_reports_what_is_on_screen() {
        let (mut session, _) = joined();
        session.apply(Action::Edit(EditKey::Insert("Plan the launch".into())), 0.0);
        let effects = session.apply(Action::Submit, 0.0);
        let started = session.active;
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::Viewing { session: Some(_) })), "started");
        let effects = session.apply(Action::NewConversation, 1.0);
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::Viewing { session: None }]),
            "a fresh one is nobody's"
        );
        let effects = session.apply(Action::SelectConversation(started), 2.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::Viewing { session: Some(id) }] if *id == started));
    }

    #[test]
    fn the_timeline_names_who_else_is_here_but_not_me() {
        let (mut session, me) = joined();
        session.apply(Action::Edit(EditKey::Insert("Plan the launch".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        let id = session.active;
        let viewers = vec![
            Viewer { user: UserId::new(), name: "Ana".into(), session: id },
            Viewer { user: me.user, name: "Olga".into(), session: id },
        ];
        session.apply(Action::Server(ServerEvent::Presence(viewers)), 1.0);
        let timeline = session.timeline(1.0);
        let item = timeline.iter().flat_map(|g| &g.items).find(|item| item.id == id).expect("listed");
        assert_eq!(item.title, "Plan the launch · Ana here");
    }
}
