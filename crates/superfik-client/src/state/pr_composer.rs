//! Opening a pull request: the confirmation sheet with its title and body,
//! filled by the user or proposed by an agent, and the command that creates
//! it. Nothing reaches `gh pr create` without `ConfirmPullRequest`.

use superfik_core::{HandoffKind, PullRequestDraft};
use superfik_protocol::ClientCommand;
use superfik_ui::TextEdit;

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrField {
    Title,
    Body,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PullRequestComposer {
    pub title: TextEdit,
    pub body: TextEdit,
    pub base: String,
    pub draft: bool,
    pub focus: Option<PrField>,
    pub drafting: bool,
    pub creating: bool,
}

impl PullRequestComposer {
    pub fn draft(&self) -> PullRequestDraft {
        PullRequestDraft {
            title: self.title.text().trim().to_owned(),
            body: self.body.text().trim().to_owned(),
            base: self.base.clone(),
            draft: self.draft,
        }
    }

    pub fn can_confirm(&self) -> bool {
        !self.title.text().trim().is_empty() && !self.drafting && !self.creating
    }

    fn field(&mut self, field: PrField) -> &mut TextEdit {
        match field {
            PrField::Title => &mut self.title,
            PrField::Body => &mut self.body,
        }
    }
}

pub const DRAFTING_PROMPT: &str =
    "Look at the changes of the current branch against its base branch and write the pull request for them.";

impl Session {
    pub fn can_open_pull_request(&self) -> bool {
        self.snapshot.repository.as_ref().is_some_and(|r| r.pull_request.is_none())
    }

    pub(super) fn propose_pull_request(&mut self) -> Vec<Effect> {
        if self.pr_composer.is_some() {
            return Vec::new();
        }
        let Some(repository) = self.snapshot.repository.as_ref() else { return Vec::new() };
        let base = repository.default_base();
        let title = self.conversation().title.clone().unwrap_or_default();
        self.popover = None;
        self.pr_composer = Some(PullRequestComposer {
            title: TextEdit::new(title),
            base,
            focus: Some(PrField::Title),
            ..PullRequestComposer::default()
        });
        Vec::new()
    }

    pub(super) fn request_pull_request_draft(&mut self, now: f64) -> Vec<Effect> {
        if self.pr_composer.is_none() {
            self.propose_pull_request();
        }
        if self.is_running() || self.pr_composer.as_ref().is_some_and(|c| c.drafting) {
            return Vec::new();
        }
        self.conversation_mut().composer.set(DRAFTING_PROMPT);
        self.pending_handoff = Some(HandoffKind::PullRequestDraft);
        let effects = self.submit(now);
        self.pending_handoff = None;
        if let Some(composer) = self.pr_composer.as_mut() {
            composer.drafting = !effects.is_empty();
        }
        effects
    }

    pub(super) fn pull_request_proposed(&mut self, draft: PullRequestDraft) {
        if let Some(composer) = self.pr_composer.as_mut() {
            composer.title.set(draft.title);
            composer.body.set(draft.body);
            composer.drafting = false;
            composer.focus = Some(PrField::Body);
        }
    }

    pub(super) fn focus_pull_request_field(&mut self, field: PrField) -> Vec<Effect> {
        if let Some(composer) = self.pr_composer.as_mut() {
            composer.focus = Some(field);
        }
        Vec::new()
    }

    pub(super) fn toggle_pull_request_draft(&mut self) -> Vec<Effect> {
        if let Some(composer) = self.pr_composer.as_mut() {
            composer.draft = !composer.draft;
        }
        Vec::new()
    }

    pub(super) fn confirm_pull_request(&mut self) -> Vec<Effect> {
        let Some(path) = self.workspace_root() else { return Vec::new() };
        let Some(composer) = self.pr_composer.as_mut() else { return Vec::new() };
        if !composer.can_confirm() {
            return Vec::new();
        }
        composer.creating = true;
        let draft = composer.draft();
        self.notice = Some(super::Notice::CreatingPullRequest);
        vec![Effect::Command(Box::new(ClientCommand::CreatePullRequest { path, draft }))]
    }

    pub(super) fn cancel_pull_request(&mut self) -> Vec<Effect> {
        self.pr_composer = None;
        Vec::new()
    }

    pub(super) fn pull_request_created(&mut self, pull_request: superfik_core::PullRequest) {
        self.pr_composer = None;
        if matches!(self.notice, Some(super::Notice::CreatingPullRequest)) {
            self.notice = None;
        }
        self.conversation_mut().pull_request = Some(pull_request);
    }

    pub(super) fn pull_request_failed(&mut self) {
        if let Some(composer) = self.pr_composer.as_mut() {
            composer.creating = false;
            composer.drafting = false;
        }
    }

    pub(super) fn edit_pull_request(&mut self, key: &super::EditKey) -> bool {
        let Some(composer) = self.pr_composer.as_mut() else { return false };
        let Some(field) = composer.focus else { return false };
        let edit = composer.field(field);
        super::composer::apply_key(edit, key.clone());
        true
    }

    pub(super) fn enter_pull_request(&mut self) -> bool {
        let Some(composer) = self.pr_composer.as_mut() else { return false };
        match composer.focus {
            Some(PrField::Title) => composer.focus = Some(PrField::Body),
            Some(PrField::Body) => composer.body.insert("\n"),
            None => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{PullRequest, Repository, WorkspaceSnapshot};
    use superfik_protocol::ServerEvent;

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey, Notice};

    fn session() -> Session {
        let mut session = Session::new(Some(PathBuf::from("/r")));
        session.snapshot = WorkspaceSnapshot {
            repository: Some(Repository {
                root: "/r".into(),
                slug: "acme/rocket".into(),
                branch: "sky".into(),
                branches: vec!["main".into(), "sky".into()],
                ..Repository::default()
            }),
        };
        session.conversation_mut().title = Some("Night sky".into());
        session
    }

    #[test]
    fn the_sheet_opens_with_a_guessed_title_and_the_default_base() {
        let mut session = session();
        session.apply(Action::ProposePullRequest, 0.0);
        let composer = session.pr_composer.as_ref().expect("sheet");
        assert_eq!(
            (composer.title.text(), composer.base.as_str(), composer.focus),
            ("Night sky", "main", Some(PrField::Title))
        );
        assert!(composer.can_confirm());
        assert!(
            !session.apply(Action::ProposePullRequest, 0.0).is_empty() || session.pr_composer.is_some(),
            "reopening keeps it"
        );
    }

    #[test]
    fn keys_edit_the_focused_field_and_enter_moves_on() {
        let mut session = session();
        session.apply(Action::ProposePullRequest, 0.0);
        session.apply(Action::Edit(EditKey::Insert("!".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        session.apply(Action::Edit(EditKey::Insert("Adds stars".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        session.apply(Action::Edit(EditKey::Insert("Tested.".into())), 0.0);
        let composer = session.pr_composer.as_ref().expect("sheet");
        assert_eq!(composer.title.text(), "Night sky!");
        assert_eq!(composer.body.text(), "Adds stars\nTested.");
        assert!(session.conversation().composer.is_empty(), "the canvas composer is untouched");
        assert!(!session.is_running(), "Enter never submits a prompt while the sheet is open");
    }

    #[test]
    fn confirming_is_the_only_path_to_creating() {
        let mut session = session();
        session.apply(Action::ProposePullRequest, 0.0);
        session.apply(Action::TogglePullRequestDraft, 0.0);
        let effects = session.apply(Action::ConfirmPullRequest, 0.0);
        assert!(matches!(
            commands(&effects)[..],
            [ClientCommand::CreatePullRequest { path, draft }]
                if path == &PathBuf::from("/r") && draft.title == "Night sky" && draft.base == "main" && draft.draft
        ));
        assert_eq!(session.notice, Some(Notice::CreatingPullRequest));
        assert!(session.apply(Action::ConfirmPullRequest, 0.0).is_empty(), "no double submit");
        let created = PullRequest { number: 3, title: "Night sky".into(), ..PullRequest::default() };
        session.apply(Action::Server(ServerEvent::PullRequestCreated(created.clone())), 0.0);
        assert_eq!(session.pr_composer, None);
        assert_eq!(session.notice, None);
        assert_eq!(session.conversation().pull_request, Some(created));
    }

    #[test]
    fn an_empty_title_cannot_be_confirmed_and_errors_keep_the_sheet() {
        let mut session = session();
        session.conversation_mut().title = None;
        session.apply(Action::ProposePullRequest, 0.0);
        assert!(session.apply(Action::ConfirmPullRequest, 0.0).is_empty());
        session.apply(Action::Edit(EditKey::Insert("Sky".into())), 0.0);
        assert!(!session.apply(Action::ConfirmPullRequest, 0.0).is_empty());
        session.apply(Action::Server(ServerEvent::error("`gh pr create` failed")), 0.0);
        let composer = session.pr_composer.as_ref().expect("still open");
        assert!(!composer.creating && composer.can_confirm());
        session.apply(Action::CancelPullRequest, 0.0);
        assert_eq!(session.pr_composer, None);
    }

    #[test]
    fn the_agent_can_write_the_draft_and_its_proposal_fills_the_sheet() {
        let mut session = session();
        let effects = session.apply(Action::RequestPullRequestDraft, 0.0);
        let [ClientCommand::SubmitPrompt { request, .. }] = commands(&effects)[..] else { panic!("expected a submit") };
        assert_eq!(request.prompt, DRAFTING_PROMPT);
        assert_eq!(request.handoff.as_ref().map(|h| h.kind.clone()), Some(HandoffKind::PullRequestDraft));
        assert!(session.pr_composer.as_ref().is_some_and(|c| c.drafting && !c.can_confirm()));
        assert!(session.apply(Action::RequestPullRequestDraft, 0.0).is_empty(), "one drafting run at a time");
        let draft = PullRequestDraft {
            title: "Stars at night".into(),
            body: "Adds stars.".into(),
            ..PullRequestDraft::default()
        };
        session.apply(Action::Server(ServerEvent::PullRequestProposed(draft)), 0.0);
        let composer = session.pr_composer.as_ref().expect("sheet");
        assert_eq!(
            (composer.title.text(), composer.body.text(), composer.drafting),
            ("Stars at night", "Adds stars.", false)
        );
        assert!(session.conversation().turns.last().is_some_and(|t| t.user.text == DRAFTING_PROMPT));
        session.apply(Action::CancelPullRequest, 0.0);
        session.conversation_mut().run = None;
        session.conversation_mut().composer.set("hi");
        let effects = session.apply(Action::Submit, 1.0);
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::SubmitPrompt { request, .. }] if request.handoff.is_none())
        );
    }

    #[test]
    fn a_cancelled_sheet_ignores_the_agents_proposal() {
        let mut session = session();
        session.apply(Action::RequestPullRequestDraft, 0.0);
        session.apply(Action::CancelPullRequest, 0.0);
        let draft = PullRequestDraft { title: "Late".into(), ..PullRequestDraft::default() };
        session.apply(Action::Server(ServerEvent::PullRequestProposed(draft)), 0.0);
        assert_eq!(session.pr_composer, None);
    }

    #[test]
    fn without_a_repository_nothing_opens() {
        let mut session = Session::new(None);
        session.apply(Action::ProposePullRequest, 0.0);
        assert_eq!(session.pr_composer, None);
    }
}
