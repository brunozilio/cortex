//! Sessions other members shared, as the organization's server lists them
//! for this workspace's project; and sharing one of ours with the project.

use superfik_core::project::ProjectRef;
use superfik_core::sharing::Visibility;
use superfik_protocol::SharedSession;

use super::{Effect, Session};

impl Session {
    pub(super) fn shared_sessions_arrived(&mut self, project: &ProjectRef, mut sessions: Vec<SharedSession>) {
        if self.project().as_ref() != Some(project) {
            return;
        }
        sessions.sort_by_key(|shared| std::cmp::Reverse(shared.session.started_unix));
        self.shared_sessions = sessions;
    }

    pub(super) fn open_shared(&mut self, id: superfik_core::ConversationId) {
        if self.conversations.iter().any(|c| c.id == id) {
            return;
        }
        let Some(shared) = self.shared_sessions.iter().find(|shared| shared.session.id == id) else { return };
        let mut conversation = super::Conversation::from_record(shared.session.clone(), &self.models);
        conversation.shared_by = Some(shared.owner_name.clone());
        self.conversations.push(conversation);
    }

    pub(super) fn continue_shared(&mut self) {
        let source = self.conversation_mut();
        let owner = source.shared_by.clone().unwrap_or_default();
        let mut copy = source.clone();
        source.composer = superfik_ui::TextEdit::default();
        copy.id = superfik_core::ConversationId::new();
        copy.shared_by = None;
        copy.visibility = Visibility::Private;
        copy.engine_session = None;
        copy.carried = Some(transcript(&copy.turns));
        self.active = copy.id;
        self.conversations.push(copy);
        self.notice = Some(super::Notice::ContinuingShared(owner));
    }

    pub(super) fn my_name(&self) -> Option<String> {
        let me = self.organization.me?;
        self.organization.members.iter().find(|member| member.user == me.user).map(|member| member.name.clone())
    }

    pub(super) fn shared_group(&self) -> Option<super::TimelineGroup> {
        let untitled = self.strings().t(superfik_i18n::Text::NewSession);
        let items: Vec<super::TimelineItem> = self
            .shared_sessions
            .iter()
            .map(|shared| super::TimelineItem {
                id: shared.session.id,
                title: format!("{} · {}", shared.session.title.as_deref().unwrap_or(&untitled), shared.owner_name),
                running: false,
                waiting: false,
                selected: shared.session.id == self.active,
                attention: None,
            })
            .collect();
        (!items.is_empty()).then(|| super::TimelineGroup {
            label: self.strings().t(superfik_i18n::Text::SharedWithYou),
            day: i64::MIN + 1,
            collapsed: false,
            items,
            archived: false,
        })
    }

    pub(super) fn toggle_sharing(&mut self) -> Vec<Effect> {
        if !self.organization.joined() {
            return Vec::new();
        }
        let conversation = self.conversation_mut();
        if conversation.shared_by.is_some() {
            return Vec::new();
        }
        conversation.visibility = match conversation.visibility {
            Visibility::Private => Visibility::Project,
            _ => Visibility::Private,
        };
        Vec::new()
    }
}

fn transcript(turns: &[superfik_core::Turn]) -> String {
    let mut text = String::from("This conversation continues one a teammate started. What was said so far:\n");
    for turn in turns {
        let who = turn.author.as_deref().unwrap_or("Teammate");
        text.push_str(&format!("\n{who}: {}\nAssistant: {}\n", turn.user.text, turn.response.text));
    }
    text.push_str("\nNow, the next message:");
    text
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{Member, OrgId, Principal, UserId};
    use superfik_protocol::{ClientCommand, PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::Action;
    use crate::state::tests::commands;

    fn shared(title: &str, started: i64) -> SharedSession {
        let session = superfik_core::SessionRecord {
            id: superfik_core::ConversationId::new(),
            title: Some(title.into()),
            started_unix: started,
            turns: vec![],
            engine_session: None,
            model: "claude-opus-5-5".into(),
            model_account: None,
            effort: superfik_core::Effort::Medium,
            archived: false,
            repositories: vec![],
            pull_request: None,
            references: vec![],
            agents: vec![],
            research_items: vec![],
            connections: vec![],
            notes: vec![],
            visibility: Visibility::Project,
        };
        SharedSession { owner: UserId::new(), owner_name: "Ana".into(), session }
    }

    fn joined() -> Session {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some(PathBuf::from("/repo"));
        session
    }

    fn rocket() -> ProjectRef {
        ProjectRef::Remote("github.com/acme/rocket".into())
    }

    fn on_rocket() -> Session {
        let mut session = joined();
        let repository =
            superfik_core::Repository { root: "/repo".into(), project: Some(rocket()), ..Default::default() };
        let snapshot = superfik_core::WorkspaceSnapshot { repository: Some(repository) };
        session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        session
    }

    #[test]
    fn shared_sessions_of_this_project_are_listed_newest_first() {
        let mut session = on_rocket();
        let sessions = vec![shared("Old", 1), shared("New", 2)];
        session.apply(Action::Server(ServerEvent::SharedSessions { project: rocket(), sessions }), 0.0);
        let titles: Vec<_> = session.shared_sessions.iter().map(|s| s.session.title.clone()).collect();
        assert_eq!(titles, [Some("New".to_owned()), Some("Old".to_owned())]);
        let other = ProjectRef::Remote("github.com/acme/other".into());
        let elsewhere = vec![shared("Else", 3)];
        session.apply(Action::Server(ServerEvent::SharedSessions { project: other, sessions: elsewhere }), 0.0);
        assert_eq!(session.shared_sessions.len(), 2);
    }

    #[test]
    fn shared_sessions_have_their_own_group_naming_who_shared_them() {
        let mut session = on_rocket();
        let sessions = vec![shared("Launch plan", 2)];
        session.apply(Action::Server(ServerEvent::SharedSessions { project: rocket(), sessions }), 0.0);
        let timeline = session.timeline(0.0);
        let group = timeline.iter().find(|group| group.label == "Shared with you").expect("shared group");
        assert_eq!(group.items[0].title, "Launch plan · Ana");
    }

    fn with_a_turn(mut shared: SharedSession) -> SharedSession {
        let user = superfik_core::UserMessage {
            text: "Plan the launch".into(),
            images: vec![],
            branch_caption: "rocket/main".into(),
        };
        let mut turn = superfik_core::Turn::new(user);
        turn.response.text = "Three steps: build, test, ship.".into();
        turn.response.activity = superfik_core::Activity::Done;
        turn.author = Some("Ana".into());
        shared.session.turns.push(turn);
        shared
    }

    #[test]
    fn writing_in_a_shared_session_continues_it_in_a_copy_of_ours() {
        let mut session = on_rocket();
        let launch = with_a_turn(shared("Launch plan", 2));
        let id = launch.session.id;
        session.apply(Action::Server(ServerEvent::SharedSessions { project: rocket(), sessions: vec![launch] }), 0.0);
        session.apply(Action::SelectConversation(id), 0.0);
        session.apply(Action::Edit(crate::state::EditKey::Insert("Add a rollback step".into())), 0.0);
        let sent = session.apply(Action::Submit, 1.0);
        assert_ne!(session.active, id, "a copy of ours");
        let copy = session.conversation();
        assert_eq!((copy.shared_by.as_ref(), copy.turns.len()), (None, 2));
        assert_eq!(copy.title.as_deref(), Some("Launch plan"));
        let prompt = commands(&sent).iter().find_map(|c| match c {
            ClientCommand::SubmitPrompt { request, .. } => Some(request.prompt.clone()),
            _ => None,
        });
        let prompt = prompt.expect("sent");
        assert!(prompt.contains("Three steps: build, test, ship.") && prompt.ends_with("Add a rollback step"));
        let theirs = session.conversations.iter().find(|c| c.id == id).expect("still theirs");
        assert_eq!((theirs.turns.len(), theirs.composer.is_empty()), (1, true));
        assert_eq!(session.notice, Some(crate::state::Notice::ContinuingShared("Ana".into())));
    }

    #[test]
    fn in_an_organization_each_turn_names_who_asked() {
        let mut session = on_rocket();
        let me = session.organization.me.expect("joined");
        let colleague = superfik_core::access::Colleague {
            user: me.user,
            name: "bia".into(),
            role: superfik_core::access::OrgRole::Member,
            pending: false,
        };
        session.apply(Action::Server(ServerEvent::Members(vec![colleague])), 0.0);
        session.apply(Action::Edit(crate::state::EditKey::Insert("Ship it".into())), 0.0);
        session.apply(Action::Submit, 1.0);
        let turn = session.conversation().turns.last().expect("turn");
        assert_eq!(turn.author.as_deref(), Some("bia"));
        assert!(turn.user.branch_caption.starts_with("bia"));
    }

    #[test]
    fn a_shared_session_opens_as_theirs_and_is_never_kept_as_ours() {
        let mut session = on_rocket();
        let launch = shared("Launch plan", 2);
        let id = launch.session.id;
        session.apply(Action::Server(ServerEvent::SharedSessions { project: rocket(), sessions: vec![launch] }), 0.0);
        let opened = session.apply(Action::SelectConversation(id), 0.0);
        assert_eq!(session.active, id);
        assert_eq!(session.conversation().shared_by.as_deref(), Some("Ana"));
        assert!(!opened.iter().any(|effect| matches!(effect, Effect::KeepSession { .. })));
        assert!(
            session
                .timeline(1.0)
                .iter()
                .all(|group| group.label == "Shared with you" || group.items.iter().all(|item| item.id != id)),
            "only in the shared group"
        );
    }

    #[test]
    fn once_git_names_the_project_the_server_is_asked_by_it() {
        let mut session = joined();
        let project = superfik_core::project::ProjectRef::Remote("github.com/acme/rocket".into());
        let repository = superfik_core::Repository {
            root: "/repo".into(),
            project: Some(project.clone()),
            ..superfik_core::Repository::default()
        };
        let snapshot = superfik_core::WorkspaceSnapshot { repository: Some(repository) };
        let effects = session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        assert!(
            commands(&effects)
                .iter()
                .any(|c| matches!(c, ClientCommand::LoadSessions { project: Some(p), .. } if *p == project))
        );
    }

    #[test]
    fn sharing_toggles_between_private_and_the_project_only_in_an_organization() {
        let share = || Action::RunCommand(superfik_core::Command::Share);
        let mut alone = Session::new(Some(PathBuf::from("/repo")));
        alone.apply(share(), 0.0);
        assert_eq!(alone.conversation().visibility, Visibility::Private);
        let mut session = joined();
        session.apply(share(), 0.0);
        assert_eq!(session.conversation().visibility, Visibility::Project);
        session.apply(share(), 0.0);
        assert_eq!(session.conversation().visibility, Visibility::Private);
    }
}
