//! Pull requests of the workspace repository: the list the Repository node
//! offers, and the one a conversation works on after checking it out.

use superfik_protocol::ClientCommand;

use super::{Effect, Session};

impl Session {
    pub(super) fn select_pull_request(&mut self, index: usize) -> Vec<Effect> {
        self.popover = None;
        let Some(pull_request) = self.pull_requests.get(index).cloned() else { return Vec::new() };
        if self.any_running() {
            self.notice = Some(super::Notice::BusySwitch);
            return Vec::new();
        }
        let Some(path) = self.workspace_root() else { return Vec::new() };
        self.notice = Some(super::Notice::CheckingOut(pull_request.number));
        let number = pull_request.number;
        self.conversation_mut().pull_request = Some(pull_request);
        vec![Effect::Command(Box::new(ClientCommand::CheckoutPullRequest { path, number }))]
    }

    pub(super) fn remove_pull_request(&mut self) -> Vec<Effect> {
        self.conversation_mut().pull_request = None;
        Vec::new()
    }

    pub(super) fn adopt_branch_pull_request(&mut self) {
        let Some(pr) = self.snapshot.repository.as_ref().and_then(|r| r.pull_request.clone()) else { return };
        let conversation = self.conversation_mut();
        if conversation.pull_request.as_ref().is_none_or(|current| current.number != pr.number) {
            conversation.pull_request = Some(pr);
        }
    }

    pub(super) fn pull_request_arrived(&mut self) {
        if matches!(self.notice, Some(super::Notice::CheckingOut(_))) {
            self.notice = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{PullRequest, Repository, WorkspaceSnapshot};
    use superfik_protocol::ServerEvent;

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey, Notice, PopoverKind};

    fn pr(number: u64) -> PullRequest {
        PullRequest {
            number,
            title: format!("PR {number}"),
            head_branch: format!("pr-{number}"),
            ..PullRequest::default()
        }
    }

    fn session() -> Session {
        let mut session = Session::new(Some(PathBuf::from("/r")));
        session.snapshot = WorkspaceSnapshot {
            repository: Some(Repository {
                root: "/r".into(),
                slug: "acme/rocket".into(),
                branch: "main".into(),
                ..Repository::default()
            }),
        };
        session.apply(Action::Server(ServerEvent::PullRequests(vec![pr(7), pr(8)])), 0.0);
        session
    }

    #[test]
    fn opening_the_picker_asks_for_the_list_and_the_list_is_kept() {
        let mut session = Session::new(Some(PathBuf::from("/r")));
        let effects = session.apply(Action::OpenPopover(PopoverKind::PullRequests), 0.0);
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::ListPullRequests { path }] if path == &PathBuf::from("/r"))
        );
        session.apply(Action::Server(ServerEvent::PullRequests(vec![pr(7)])), 0.0);
        assert_eq!(session.pull_requests, vec![pr(7)]);
    }

    #[test]
    fn choosing_a_pull_request_checks_it_out_and_shows_its_node() {
        let mut session = session();
        session.popover = Some(PopoverKind::PullRequests);
        let effects = session.apply(Action::SelectPullRequest(1), 0.0);
        assert!(matches!(
            commands(&effects)[..],
            [ClientCommand::CheckoutPullRequest { path, number: 8 }] if path == &PathBuf::from("/r")
        ));
        assert_eq!(session.popover, None);
        assert_eq!(session.conversation().pull_request, Some(pr(8)));
        assert_eq!(session.notice, Some(Notice::CheckingOut(8)));
        let mut snapshot = session.snapshot.clone();
        if let Some(repo) = snapshot.repository.as_mut() {
            repo.branch = "pr-8".into();
        }
        session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        assert_eq!(session.notice, None);
        assert_eq!(session.branch.as_deref(), Some("pr-8"));
    }

    #[test]
    fn checkout_waits_for_running_sessions_and_ignores_unknown_rows() {
        let mut session = session();
        assert!(session.apply(Action::SelectPullRequest(5), 0.0).is_empty());
        session.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        assert!(session.apply(Action::SelectPullRequest(0), 0.0).is_empty());
        assert_eq!(session.notice, Some(Notice::BusySwitch));
        assert_eq!(session.conversation().pull_request, None);
    }

    #[test]
    fn a_pull_request_on_the_branch_is_adopted_from_the_snapshot() {
        let mut session = session();
        let mut snapshot = session.snapshot.clone();
        if let Some(repo) = snapshot.repository.as_mut() {
            repo.pull_request = Some(pr(3));
        }
        session.apply(Action::Server(ServerEvent::Workspace(snapshot.clone())), 0.0);
        assert_eq!(session.conversation().pull_request, Some(pr(3)));
        session.apply(Action::RemovePullRequest, 0.0);
        session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        assert_eq!(session.conversation().pull_request, Some(pr(3)), "the snapshot still names it");
    }

    #[test]
    fn a_refused_checkout_keeps_the_node_and_shows_the_error() {
        let mut session = session();
        session.apply(Action::SelectPullRequest(0), 0.0);
        session.apply(Action::Server(ServerEvent::error("`gh pr checkout 7` failed")), 0.0);
        assert!(matches!(&session.notice, Some(Notice::Raw(m)) if m.contains("checkout 7")));
        assert_eq!(session.conversation().pull_request, Some(pr(7)));
        session.apply(Action::RemovePullRequest, 0.0);
        assert_eq!(session.conversation().pull_request, None);
    }
}
