//! Comments on shared sessions (`superfik_core::comments`): asked for
//! when such a session comes on screen, left with `/comment <text>`, and
//! shown in the transcript after the turn they follow.

use superfik_core::comments::Comment;
use superfik_core::{ConversationId, command};
use superfik_protocol::ClientCommand;

use super::{Effect, Notice, Session};

impl Session {
    pub fn comments_on(&self, session: ConversationId) -> &[Comment] {
        self.comments.get(&session).map_or(&[], Vec::as_slice)
    }

    pub(super) fn load_comments(&self, session: ConversationId) -> Vec<Effect> {
        match self.project() {
            Some(project) if self.organization.joined() => {
                vec![Effect::Command(Box::new(ClientCommand::LoadComments { project, session }))]
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn run_comment(&mut self) -> Option<Vec<Effect>> {
        let text = command::argument(self.conversation().composer.text(), "comment")?.to_owned();
        self.conversation_mut().composer.take();
        let (Some(project), true) = (self.project(), self.organization.joined()) else {
            self.notice = Some(Notice::CommentNeedsOrganization);
            return Some(Vec::new());
        };
        let c = self.conversation();
        if c.started_unix.is_none() && c.shared_by.is_none() {
            self.notice = Some(Notice::CommentNeedsOrganization);
            return Some(Vec::new());
        }
        let after_turn = c.turns.len().saturating_sub(1);
        let command = ClientCommand::AddComment { project, session: c.id, after_turn, text };
        Some(vec![Effect::Command(Box::new(command))])
    }

    pub(super) fn comments_arrived(&mut self, session: ConversationId, comments: Vec<Comment>) {
        let before = self.comments_on(session).len();
        let me = self.organization.me.map(|me| me.user);
        if let Some(newest) = comments.last().filter(|c| comments.len() > before && Some(c.author) != me) {
            self.notice = Some(Notice::Commented(newest.author_name.clone()));
        }
        self.comments.insert(session, comments);
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::project::ProjectRef;
    use superfik_core::{Member, OrgId, Principal, UserId};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey};

    fn on_rocket() -> (Session, Member) {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some("/repo".into());
        let project = Some(ProjectRef::Remote("github.com/acme/rocket".into()));
        let repository = superfik_core::Repository { root: "/repo".into(), project, ..Default::default() };
        let snapshot = superfik_core::WorkspaceSnapshot { repository: Some(repository) };
        session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        (session, me)
    }

    fn comment(author: UserId, name: &str) -> Comment {
        Comment { author, author_name: name.into(), at_unix: 1, after_turn: 0, text: "Ship it".into() }
    }

    #[test]
    fn a_started_session_asks_for_its_comments_and_takes_new_ones() {
        let (mut session, _) = on_rocket();
        session.apply(Action::Edit(EditKey::Insert("Plan the launch".into())), 0.0);
        let effects = session.apply(Action::Submit, 0.0);
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::LoadComments { .. })));
        let effects = {
            session.apply(Action::Edit(EditKey::Insert("/comment Looks right".into())), 1.0);
            session.apply(Action::Submit, 1.0)
        };
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::AddComment { text, after_turn: 0, .. }] if text == "Looks right")
        );
        assert_eq!(session.conversation().turns.len(), 1, "nothing went to the model");
    }

    #[test]
    fn someone_elses_comment_is_announced_and_kept() {
        let (mut session, me) = on_rocket();
        let id = session.active;
        let mine = vec![comment(me.user, "Olga")];
        session.apply(Action::Server(ServerEvent::Comments { session: id, comments: mine.clone() }), 0.0);
        assert_eq!(session.notice, None, "my own");
        let mut both = mine;
        both.push(comment(UserId::new(), "Ana"));
        session.apply(Action::Server(ServerEvent::Comments { session: id, comments: both }), 0.0);
        assert_eq!(session.notice, Some(Notice::Commented("Ana".into())));
        assert_eq!(session.comments_on(id).len(), 2);
    }
}
