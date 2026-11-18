use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::team::{LEAD, Mail, Member, TeamConfig, file_name};
use platform::store::Document;

pub mod launch;
pub mod lead_inbox;

pub const TEAM_VARIABLE: &str = concat!(brand::env_prefix!(), "_TEAM_NAME");
pub const AGENT_VARIABLE: &str = concat!(brand::env_prefix!(), "_AGENT_NAME");

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TeamError {
    #[error("team {0} is gone")]
    Gone(String),
    #[error("{0}")]
    Name(String),
    #[error("{0}")]
    Unsaved(String),
}

impl From<TeamError> for String {
    fn from(error: TeamError) -> Self {
        error.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    pub name: String,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    pub team: Team,
    pub name: String,
}

impl Team {
    pub fn root(home: &Path) -> PathBuf {
        home.join(brand::DIR).join("teams")
    }

    pub fn named(home: &Path, name: &str) -> Self {
        Self { name: name.to_owned(), dir: Self::root(home).join(file_name(name)) }
    }

    pub fn of_session(home: &Path, session: &str) -> Self {
        let short: String = session.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
        Self::named(home, &format!("team-{short}"))
    }

    pub fn config_path(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    fn config(&self) -> Document {
        Document::at(self.config_path())
    }

    pub fn read(&self) -> Option<TeamConfig> {
        self.config().read().ok().flatten()
    }

    pub fn exists(&self) -> bool {
        self.config_path().is_file()
    }

    pub fn open(&self, lead_session: &str) -> Result<TeamConfig, TeamError> {
        std::fs::create_dir_all(self.dir.join("inboxes")).map_err(|error| TeamError::Unsaved(error.to_string()))?;
        self.config()
            .update(|config: &mut TeamConfig| {
                if config.name.is_empty() {
                    config.name.clone_from(&self.name);
                    config.created_at = now_ms();
                    lead_session.clone_into(&mut config.lead_session_id);
                }
                config.clone()
            })
            .map_err(|error| TeamError::Unsaved(error.to_string()))
    }

    pub fn join(&self, wanted: &str, mut member: Member) -> Result<String, TeamError> {
        self.config()
            .try_update(|config: &mut TeamConfig| {
                let name = domain::team::free_name(wanted, &config.names()).map_err(TeamError::Name)?;
                member.name.clone_from(&name);
                member.joined_at = now_ms();
                member.active = true;
                config.members.push(member);
                Ok(name)
            })
            .map_err(|error| TeamError::Unsaved(error.to_string()))?
    }

    pub fn left(&self, name: &str) {
        let _ = self.config().update(|config: &mut TeamConfig| {
            for member in config.members.iter_mut().filter(|member| member.name == name) {
                member.active = false;
            }
        });
    }

    fn mailbox(&self, agent: &str) -> Document {
        Document::at(self.dir.join("inboxes").join(format!("{}.json", file_name(agent))))
    }

    pub fn knows(&self, agent: &str) -> bool {
        agent == LEAD || self.read().is_some_and(|config| config.members.iter().any(|member| member.name == agent))
    }

    pub fn send(&self, to: &str, from: &str, text: &str, summary: Option<&str>) -> Result<(), TeamError> {
        if !self.exists() {
            return Err(TeamError::Gone(self.name.clone()));
        }
        let mail = Mail {
            from: from.to_owned(),
            text: text.to_owned(),
            timestamp: now_ms(),
            read: false,
            summary: summary.map(str::to_owned),
            msg_id: uuid::Uuid::new_v4().to_string(),
        };
        self.mailbox(to)
            .update(|mails: &mut Vec<Mail>| mails.push(mail))
            .map_err(|error| TeamError::Unsaved(error.to_string()))
    }

    pub fn unread(&self, agent: &str) -> Vec<Mail> {
        self.mailbox(agent)
            .update(|mails: &mut Vec<Mail>| {
                let fresh: Vec<Mail> = mails.iter().filter(|mail| !mail.read).cloned().collect();
                mails.iter_mut().for_each(|mail| mail.read = true);
                fresh
            })
            .unwrap_or_default()
    }
}

impl Me {
    pub fn from_env(home: &Path) -> Option<Self> {
        let team = std::env::var(TEAM_VARIABLE).ok().filter(|name| !name.is_empty())?;
        let name = std::env::var(AGENT_VARIABLE).ok().filter(|name| !name.is_empty())?;
        Some(Self { team: Team::named(home, &team), name })
    }
}

pub fn tasks_file(home: &Path, lead_session: &str) -> PathBuf {
    home.join(brand::DIR).join("tasks").join(file_name(lead_session)).join("tasks.json")
}

pub fn current(home: Option<&Path>, session: &str, teammate: Option<&Me>) -> Option<Me> {
    if let Some(me) = teammate {
        return Some(me.clone());
    }
    let team = Team::of_session(home?, session);
    team.exists().then(|| Me { team, name: LEAD.to_owned() })
}

pub fn task_list(home: Option<&Path>, session: &str, teammate: Option<&Me>) -> Option<PathBuf> {
    let home = home?;
    match teammate {
        Some(me) => me.team.read().map(|config| tasks_file(home, &config.lead_session_id)),
        None => Some(tasks_file(home, session)),
    }
}

impl Team {
    pub fn departed(&self, name: &str) {
        self.left(name);
        let released = self.release_tasks(name);
        if !released.is_empty() {
            let note = format!(
                "{name} has stopped. Its open tasks ({}) are back in the list without an owner: give them to another \
                 teammate with TaskUpdate.",
                released.join(", ")
            );
            let _ = self.send(LEAD, name, &note, Some("stopped with open tasks"));
        }
    }

    pub fn shutting_down(&self, agent: &str) -> bool {
        let asked = |mail: &Mail| {
            mail.from == LEAD
                && matches!(
                    domain::team::Protocol::of(&mail.text),
                    Some(domain::team::Protocol::ShutdownRequest { .. })
                )
        };
        self.mailbox(agent)
            .read::<Vec<Mail>>()
            .ok()
            .flatten()
            .is_some_and(|mails| mails.iter().any(|mail| !mail.read && asked(mail)))
    }

    pub fn take_answer(&self, agent: &str, request_id: &str) -> Option<domain::Decision> {
        let answered = |mail: &Mail| {
            let answer = domain::team::Protocol::of(&mail.text).filter(|_| mail.from == LEAD)?;
            domain::team::answers(&answer, request_id).then(|| domain::team::decided(&answer)).flatten()
        };
        self.mailbox(agent)
            .update(|mails: &mut Vec<Mail>| {
                let at = mails.iter().position(|mail| answered(mail).is_some())?;
                answered(&mails.remove(at))
            })
            .ok()
            .flatten()
    }

    pub fn home(&self) -> Option<&Path> {
        self.dir.parent()?.parent()?.parent()
    }

    pub fn release_tasks(&self, owner: &str) -> Vec<String> {
        let (Some(home), Some(config)) = (self.home(), self.read()) else { return Vec::new() };
        Document::at(tasks_file(home, &config.lead_session_id))
            .update(|board: &mut domain::task_board::Board| board.release(owner))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(agent_type: &str) -> Member {
        Member { agent_type: agent_type.into(), cwd: "/work".into(), ..Member::default() }
    }

    #[test]
    fn a_team_is_opened_once_and_teammates_join_under_unique_names() {
        let home = tempfile::tempdir().expect("home");
        let team = Team::of_session(home.path(), "4f1c-9a2b-77");
        assert_eq!(team.name, "team-4f1c9a2b");
        let opened = team.open("4f1c-9a2b-77").expect("open");
        assert_eq!(opened.lead_session_id, "4f1c-9a2b-77");
        assert_eq!(team.open("other").expect("again").lead_session_id, "4f1c-9a2b-77", "opened once");
        assert_eq!(team.join("tester", member("general")), Ok("tester".into()));
        assert_eq!(team.join("tester", member("general")), Ok("tester-2".into()));
        assert!(team.knows("tester-2") && team.knows(LEAD) && !team.knows("ghost"));
        team.left("tester");
        let config = team.read().expect("config");
        assert_eq!(config.members.iter().filter(|member| member.active).count(), 1);
    }

    #[test]
    fn mail_is_read_once_in_the_order_it_was_sent() {
        let home = tempfile::tempdir().expect("home");
        let team = Team::named(home.path(), "t");
        assert!(team.send("tester", LEAD, "first", None).is_err(), "no team, no mail");
        team.open("s").expect("open");
        team.send("tester", LEAD, "first", None).expect("send");
        team.send("tester", "other", "second", Some("two")).expect("send");
        let unread = team.unread("tester");
        assert_eq!(unread.iter().map(|mail| mail.text.as_str()).collect::<Vec<_>>(), ["first", "second"]);
        assert_eq!(unread[1].summary.as_deref(), Some("two"));
        assert!(team.unread("tester").is_empty(), "read once");
        assert!(team.unread(LEAD).is_empty());
    }

    #[test]
    fn a_teammate_that_leaves_gives_its_open_tasks_back_to_the_team_s_list() {
        let home = tempfile::tempdir().expect("home");
        let team = Team::of_session(home.path(), "lead-1");
        team.open("lead-1").expect("open");
        assert_eq!(team.home(), Some(home.path()));
        let list = Document::at(tasks_file(home.path(), "lead-1"));
        list.update(|board: &mut domain::task_board::Board| {
            board.create("Fix", "", None).expect("task");
            let claimed = domain::task_board::Change { owner: Some("tester".into()), ..Default::default() };
            board.update("1", claimed, "tester").expect("claim");
        })
        .expect("list");

        assert_eq!(team.release_tasks("tester"), ["1"]);

        let board: domain::task_board::Board = list.read().expect("read").expect("board");
        assert_eq!(board.listed()[0]["owner"], serde_json::Value::Null);
        assert_eq!(task_list(Some(home.path()), "lead-1", None), Some(tasks_file(home.path(), "lead-1")));
        assert_eq!(current(Some(home.path()), "lead-1", None).map(|me| me.name), Some(LEAD.to_owned()));
        assert!(current(Some(home.path()), "someone-else", None).is_none());
        assert!(task_list(None, "lead-1", None).is_none());
    }
}
