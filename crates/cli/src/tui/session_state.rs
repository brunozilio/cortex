use std::collections::BTreeSet;
use std::path::PathBuf;

use app::{Effort, Headless, Model, PermissionMode, Session, SessionId};
use domain::model_roles::ModelRoles;

use super::settings::{AUTO_EFFORT, Saved};

pub struct SessionState {
    live: Option<Session>,
    pub title: Option<String>,
    pub window: Option<u64>,
    pub(super) output_style: Option<String>,
    pub(super) roles: ModelRoles,
    pub(super) fallbacks: Vec<String>,
    pub headless: Headless,
    pub fork_resumed: bool,
    added_dirs: Vec<PathBuf>,
    pub turned_off: BTreeSet<String>,
    pub(super) wanted: Option<String>,
    pub(super) resume: Option<SessionId>,
    mode_given: bool,
    mode: PermissionMode,
    effort: Effort,
    auto_effort: bool,
}

impl SessionState {
    pub fn new(wanted: Option<String>, mode: Option<PermissionMode>) -> Self {
        Self {
            live: None,
            title: None,
            window: None,
            output_style: None,
            roles: ModelRoles::default(),
            fallbacks: Vec::new(),
            headless: Headless::default(),
            fork_resumed: false,
            added_dirs: Vec::new(),
            turned_off: BTreeSet::new(),
            wanted,
            resume: None,
            mode_given: mode.is_some(),
            mode: mode.unwrap_or_default(),
            effort: Effort::Medium,
            auto_effort: false,
        }
    }

    pub fn live(&self) -> Option<&Session> {
        self.live.as_ref()
    }

    pub fn live_mut(&mut self) -> Option<&mut Session> {
        self.live.as_mut()
    }

    pub fn is_some(&self) -> bool {
        self.live.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.live.is_none()
    }

    pub fn trade_live(&mut self, other: &mut Option<Session>) {
        std::mem::swap(&mut self.live, other);
    }

    pub fn id(&self) -> Option<SessionId> {
        self.live.as_ref().and_then(Session::id)
    }

    pub fn output_style(&self) -> Option<&str> {
        self.output_style.as_deref()
    }

    pub fn roles(&self) -> &ModelRoles {
        &self.roles
    }

    pub fn fallbacks(&self) -> &[String] {
        &self.fallbacks
    }

    pub fn set_role(&mut self, role: domain::model_roles::ModelRole, model: Option<String>) {
        self.roles.set(role, model);
        if let Some(session) = &mut self.live {
            session.set_roles(self.roles.clone());
        }
    }

    pub fn set_fallbacks(&mut self, chain: Vec<String>) {
        self.fallbacks = chain;
        if let Some(session) = &mut self.live {
            session.set_fallbacks(self.fallbacks.clone());
        }
    }

    pub fn model(&self) -> Option<&Model> {
        self.live.as_ref().map(Session::model)
    }

    pub fn mode(&self) -> PermissionMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
        if let Some(session) = &mut self.live {
            session.set_permission(mode);
        }
    }

    pub fn effort(&self) -> Effort {
        self.effort
    }

    pub fn effort_label(&self) -> &'static str {
        if self.auto_effort { AUTO_EFFORT } else { self.effort.cli_value() }
    }

    pub fn set_effort(&mut self, effort: Effort) {
        self.effort = effort;
        self.auto_effort = false;
        if let Some(session) = &mut self.live {
            session.set_effort(effort);
        }
    }

    pub fn set_auto_effort(&mut self) {
        self.auto_effort = true;
        if let Some(session) = &mut self.live {
            session.set_auto_pick();
        }
    }

    pub fn set_output_style(&mut self, style: String) {
        if let Some(session) = &mut self.live {
            session.set_output_style(Some(style.clone()));
        }
        self.output_style = Some(style);
    }

    pub fn added_dirs(&self) -> &[PathBuf] {
        &self.added_dirs
    }

    pub fn add_dir(&mut self, dir: PathBuf) {
        if let Some(session) = &mut self.live {
            session.add_dir(dir.clone());
        }
        if !self.added_dirs.contains(&dir) {
            self.added_dirs.push(dir);
        }
    }

    pub(super) fn take_saved_choices(&mut self, saved: &Saved) {
        if !self.mode_given {
            self.mode = saved.mode.unwrap_or(self.mode);
        }
        self.effort = saved.effort.unwrap_or(self.effort);
        self.auto_effort = saved.auto;
    }

    pub(super) fn adopt(&mut self, mut session: Session) {
        session.set_permission(self.mode);
        session.set_effort(self.effort);
        if self.auto_effort {
            session.set_auto_pick();
        }
        for dir in &self.added_dirs {
            session.add_dir(dir.clone());
        }
        session.set_output_style(self.output_style.clone());
        session.set_roles(self.roles.clone());
        session.set_fallbacks(self.fallbacks.clone());
        session.set_headless(self.headless.clone());
        self.live = Some(session);
    }
}

#[cfg(test)]
mod tests {
    use agent_engine::fake::FakeEngine;

    use super::*;

    fn session(data: &std::path::Path) -> Session {
        let model = Model::new("keyed:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model.clone()], Vec::new());
        crate::tui::tests::client(&engine, data).session(data.to_owned(), model)
    }

    #[test]
    fn what_was_chosen_before_the_session_started_reaches_it_when_it_starts() {
        let data = tempfile::tempdir().expect("data");
        let mut state = SessionState::new(None, Some(PermissionMode::Plan));
        state.set_effort(Effort::High);
        state.add_dir(data.path().to_owned());
        assert!(state.is_none() && state.id().is_none() && state.model().is_none());

        state.adopt(session(data.path()));
        let live = state.live().expect("a session");
        assert_eq!(live.permission_mode(), PermissionMode::Plan);
        assert_eq!(live.effort_level(), Effort::High);
        assert_eq!(live.added_dirs(), state.added_dirs());
    }

    #[test]
    fn a_choice_changed_later_reaches_the_live_session_at_once() {
        let data = tempfile::tempdir().expect("data");
        let mut state = SessionState::new(None, None);
        state.adopt(session(data.path()));
        state.set_mode(PermissionMode::Bypass);
        state.set_effort(Effort::Low);
        state.add_dir(data.path().to_owned());
        state.add_dir(data.path().to_owned());
        let live = state.live().expect("a session");
        assert_eq!((state.mode(), live.permission_mode()), (PermissionMode::Bypass, PermissionMode::Bypass));
        assert_eq!((state.effort(), live.effort_level()), (Effort::Low, Effort::Low));
        assert_eq!(state.added_dirs(), [data.path().to_owned()], "a folder is added once");
        state.set_auto_effort();
        assert_eq!(state.effort_label(), AUTO_EFFORT);
        state.set_effort(Effort::Max);
        assert_eq!(state.effort_label(), Effort::Max.cli_value(), "a level ends the automatic pick");
    }

    #[test]
    fn a_mode_given_on_the_command_line_is_not_replaced_by_the_saved_one() {
        let saved = Saved { mode: Some(PermissionMode::Bypass), effort: Some(Effort::High), ..Saved::default() };
        let mut given = SessionState::new(None, Some(PermissionMode::Plan));
        given.take_saved_choices(&saved);
        assert_eq!((given.mode(), given.effort()), (PermissionMode::Plan, Effort::High));
        let mut free = SessionState::new(None, None);
        free.take_saved_choices(&saved);
        assert_eq!(free.mode(), PermissionMode::Bypass);
    }
}
