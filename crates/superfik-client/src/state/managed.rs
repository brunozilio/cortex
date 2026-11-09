//! What a company's IT set for this app through device management (MDM):
//! whether the app works only as a member of an organization. The shell
//! reads it at launch (`app::managed`) and dispatches `Action::Managed`.
//! The app connects to the team's server from Settings › Server.

use super::{Effect, Notice, Session};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Managed {
    pub require_organization: bool,
}

impl Session {
    pub(super) fn managed(&mut self, managed: Managed) -> Vec<Effect> {
        self.managed = managed;
        Vec::new()
    }

    pub(super) fn awaiting_organization(&mut self) -> bool {
        let waiting = self.managed.require_organization && !self.organization.joined();
        if waiting {
            self.notice = Some(Notice::OrganizationRequired);
        }
        waiting
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Action, EditKey};

    #[test]
    fn nothing_is_sent_before_joining_when_it_is_required() {
        let mut session = Session::new(Some("/repo".into()));
        session.apply(Action::Managed(Managed { require_organization: true }), 0.0);
        session.apply(Action::Edit(EditKey::Insert("Ship it".into())), 0.0);
        let effects = session.apply(Action::Submit, 1.0);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Command(_))));
        assert_eq!(session.notice, Some(Notice::OrganizationRequired));
    }
}
