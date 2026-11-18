use std::time::Duration;

use app::team::Me;
use domain::team::{LEAD, Protocol, shown};

const POLL: Duration = Duration::from_millis(500);

pub struct Mailbox {
    me: Me,
    answered_once: bool,
    poll: Duration,
}

impl Mailbox {
    pub fn new(me: Me) -> Self {
        Self { me, answered_once: false, poll: POLL }
    }

    fn went_idle(&self) {
        let idle =
            Protocol::IdleNotification { from: self.me.name.clone(), idle_reason: "available".into(), summary: None };
        if let Err(error) = self.me.team.send(LEAD, &self.me.name, &idle.text(), Some("idle")) {
            tracing::warn!(%error, "the lead could not be told this teammate is idle");
        }
    }

    fn shut_down(&self, request_id: String) {
        let approved = Protocol::ShutdownApproved { request_id, from: self.me.name.clone() };
        let _ = self.me.team.send(LEAD, &self.me.name, &approved.text(), Some("shutting down"));
    }
}

impl Iterator for Mailbox {
    type Item = Result<String, String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.answered_once {
            self.went_idle();
        }
        self.answered_once = true;
        loop {
            if !self.me.team.exists() {
                return None;
            }
            let mut work = Vec::new();
            for mail in self.me.team.unread(&self.me.name) {
                if let Some(Protocol::ShutdownRequest { request_id, .. }) = Protocol::of(&mail.text) {
                    self.shut_down(request_id);
                    return None;
                }
                work.push(if mail.from == LEAD && Protocol::of(&mail.text).is_none() {
                    mail.text.clone()
                } else {
                    shown(&mail)
                });
            }
            if !work.is_empty() {
                return Some(Ok(work.join("\n\n")));
            }
            std::thread::sleep(self.poll);
        }
    }
}

#[cfg(test)]
mod tests {
    use app::team::Team;

    use super::*;

    fn rig() -> (tempfile::TempDir, Team, Mailbox) {
        let home = tempfile::tempdir().expect("home");
        let team = Team::named(home.path(), "t");
        team.open("s").expect("open");
        let me = Me { team: team.clone(), name: "tester".into() };
        let mailbox = Mailbox { poll: Duration::from_millis(5), ..Mailbox::new(me) };
        (home, team, mailbox)
    }

    #[test]
    fn a_teammate_takes_the_lead_s_task_then_goes_idle_and_takes_the_next_message() {
        let (_home, team, mut mailbox) = rig();
        team.send("tester", LEAD, "fix the tests", None).expect("send");
        assert_eq!(mailbox.next(), Some(Ok("fix the tests".into())));
        assert!(team.unread(LEAD).is_empty(), "not idle before its first answer");

        team.send("tester", "reviewer", "look at x", Some("x")).expect("send");
        let next = mailbox.next().expect("more work").expect("text");
        assert!(next.starts_with("<teammate-message teammate_id=\"reviewer\""), "{next}");
        let told = team.unread(LEAD);
        assert_eq!(told.len(), 1);
        assert!(
            matches!(Protocol::of(&told[0].text), Some(Protocol::IdleNotification { from, .. }) if from == "tester")
        );
    }

    #[test]
    fn a_shutdown_request_is_approved_and_ends_the_teammate() {
        let (_home, team, mut mailbox) = rig();
        let asked = Protocol::ShutdownRequest { request_id: "r1".into(), from: LEAD.into(), reason: None };
        team.send("tester", LEAD, &asked.text(), None).expect("send");

        assert_eq!(mailbox.next(), None);

        let told = team.unread(LEAD);
        assert_eq!(
            Protocol::of(&told[0].text),
            Some(Protocol::ShutdownApproved { request_id: "r1".into(), from: "tester".into() })
        );
    }
}
