use std::time::Duration;

use app::team::Me;
use domain::team::{LEAD, Protocol, shown};

const POLL: Duration = Duration::from_millis(500);

pub fn spec_of(cli: &mut crate::Cli) -> Result<Option<domain::team::TeammateSpec>, String> {
    let Some(crate::Action::Teammate { spec: Some(path) }) = &cli.command else { return Ok(None) };
    let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let spec: domain::team::TeammateSpec =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    cli.cwd = Some(spec.cwd.clone().into());
    cli.model = Some(spec.model.clone());
    cli.mode = Some(domain::command::mode_named(&spec.mode).ok_or_else(|| format!("unknown mode {}", spec.mode))?);
    cli.append_system_prompt = Some(spec.instructions.clone());
    Ok(Some(spec))
}

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

    pub fn forward(&self, ask: &app::AgentAsk) -> Option<app::Decision> {
        let nanos =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_nanos());
        let request_id = format!("{}-{nanos:x}", self.me.name);
        let from = self.me.name.clone();
        let (request, summary) = match &ask.kind {
            app::AskKind::Permission { tool, subject, rememberable, rule } => (
                Protocol::PermissionRequest {
                    request_id: request_id.clone(),
                    from,
                    tool_name: tool.clone(),
                    description: subject.clone(),
                    rememberable: *rememberable,
                    rule: rule.clone(),
                },
                format!("may I use {tool}?"),
            ),
            app::AskKind::Plan { plan } => (
                Protocol::PlanApprovalRequest { request_id: request_id.clone(), from, plan: plan.clone() },
                "plan ready for approval".to_owned(),
            ),
            app::AskKind::Questions(_) => return None,
        };
        self.me.team.send(LEAD, &self.me.name, &request.text(), Some(&summary)).ok()?;
        loop {
            if let Some(decision) = self.me.team.take_answer(&self.me.name, &request_id) {
                return Some(decision);
            }
            if !self.me.team.exists() || self.me.team.shutting_down(&self.me.name) {
                return Some(app::Decision::Deny);
            }
            std::thread::sleep(self.poll);
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
    fn a_permission_prompt_goes_to_the_lead_and_the_lead_s_answer_settles_it() {
        let (_home, team, mailbox) = rig();
        let ask = app::AgentAsk {
            id: app::AskId("ask-1".into()),
            kind: app::AskKind::Permission {
                tool: "Bash".into(),
                subject: "npm run deploy".into(),
                rememberable: true,
                rule: None,
            },
        };
        let answered = |decision: app::Decision| {
            let lead = team.clone();
            std::thread::spawn(move || {
                for _ in 0..400 {
                    let waiting = lead.unread(LEAD);
                    if let Some(asked) = waiting.first().and_then(|mail| domain::team::TeammateAsk::of("t", mail)) {
                        lead.send("tester", LEAD, &asked.answer(&decision).text(), None).expect("answer");
                        return asked;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                panic!("nothing was asked")
            })
        };

        let lead = answered(app::Decision::AllowAlways);
        assert_eq!(mailbox.forward(&ask), Some(app::Decision::AllowAlways), "always reaches the teammate");
        assert!(matches!(lead.join().expect("lead").asked, domain::team::Asked::Permission { rememberable: true, .. }));

        let approved = app::Decision::Approve(app::PermissionMode::AcceptEdits);
        let lead = answered(approved.clone());
        let plan = app::AgentAsk { kind: app::AskKind::Plan { plan: "1. edit".into() }, ..ask.clone() };
        assert_eq!(mailbox.forward(&plan), Some(approved), "a plan is approved by the lead's user");
        assert!(matches!(lead.join().expect("lead").asked, domain::team::Asked::Plan { .. }));
    }

    #[test]
    fn a_teammate_asked_to_shut_down_stops_waiting_for_an_answer() {
        let (_home, team, mailbox) = rig();
        let asked = Protocol::ShutdownRequest { request_id: "r1".into(), from: LEAD.into(), reason: None };
        team.send("tester", LEAD, &asked.text(), None).expect("send");
        let ask = app::AgentAsk {
            id: app::AskId("ask-1".into()),
            kind: app::AskKind::Permission {
                tool: "Bash".into(),
                subject: "x".into(),
                rememberable: false,
                rule: None,
            },
        };

        assert_eq!(mailbox.forward(&ask), Some(app::Decision::Deny));
        assert!(team.shutting_down("tester"), "the request is left for the mailbox to act on");
    }

    #[test]
    fn a_spec_file_gives_a_pane_s_teammate_its_folder_model_mode_and_instructions() {
        use clap::Parser;
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("tester.json");
        let spec = domain::team::TeammateSpec {
            team: "team-1".into(),
            name: "tester".into(),
            cwd: "/work".into(),
            model: "m1".into(),
            mode: "edits".into(),
            instructions: "# Team coordination".into(),
        };
        std::fs::write(&path, serde_json::to_string(&spec).expect("json")).expect("write");
        let mut cli = crate::Cli::parse_from([brand::COMMAND, "teammate", "--spec", &path.display().to_string()]);

        assert_eq!(spec_of(&mut cli), Ok(Some(spec)));

        assert_eq!(cli.cwd, Some("/work".into()));
        assert_eq!((cli.model.as_deref(), cli.mode), (Some("m1"), Some(app::PermissionMode::AcceptEdits)));
        assert_eq!(cli.append_system_prompt.as_deref(), Some("# Team coordination"));
        let mut plain = crate::Cli::parse_from([brand::COMMAND, "teammate"]);
        assert_eq!(spec_of(&mut plain), Ok(None));
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
