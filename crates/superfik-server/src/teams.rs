//! The organization's teams (`superfik_core::sharing::Team`): owners and
//! admins make them and say who is in them; every member lists them, and
//! sessions and memory can be shared with one. A change goes to everyone.

use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::sharing::Team;
use superfik_core::{Principal, TeamId};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent};

use crate::Organization;

pub trait TeamStore: Send + Sync {
    fn load(&self) -> Vec<Team>;
    fn store(&self, teams: &[Team]) -> std::io::Result<()>;
}

#[derive(Debug)]
pub struct FileTeams {
    file: PathBuf,
    writing: Mutex<()>,
}

impl FileTeams {
    pub fn new(file: PathBuf) -> Self {
        Self { file, writing: Mutex::new(()) }
    }
}

impl TeamStore for FileTeams {
    fn load(&self) -> Vec<Team> {
        std::fs::read(&self.file).ok().and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default()
    }

    fn store(&self, teams: &[Team]) -> std::io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = self.file.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(teams).map_err(std::io::Error::other)?)?;
        std::fs::rename(temporary, &self.file)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryTeams(Mutex<Vec<Team>>);

impl TeamStore for InMemoryTeams {
    fn load(&self) -> Vec<Team> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn store(&self, teams: &[Team]) -> std::io::Result<()> {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = teams.to_vec();
        Ok(())
    }
}

pub fn answer(org: &Organization, by: &Principal, command: &ClientCommand) -> Option<Vec<Addressed>> {
    let to_sender = |event| Addressed { to: Recipient::Principal(by.clone()), event };
    let refused = |why: &str| Some(vec![to_sender(ServerEvent::error(why))]);
    if !matches!(
        command,
        ClientCommand::ListTeams
            | ClientCommand::CreateTeam { .. }
            | ClientCommand::RemoveTeam { .. }
            | ClientCommand::SetTeamMember { .. }
    ) {
        return None;
    }
    let Principal::Member(member) = by else { return refused("not allowed") };
    let Some(role) = org.directory.role(member).filter(|_| org.reads(member)) else { return refused("not allowed") };
    let _editing = EDITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut teams = org.teams.load();
    if matches!(command, ClientCommand::ListTeams) {
        return Some(vec![to_sender(ServerEvent::Teams(teams))]);
    }
    if !superfik_core::access::administers(role) {
        return refused("only owners and admins change teams");
    }
    match command {
        ClientCommand::CreateTeam { name } => {
            let name = name.trim();
            if name.is_empty() || superfik_core::sharing::team_named(&teams, name).is_some() {
                return refused("teams need a name of their own");
            }
            teams.push(Team { id: TeamId::new(), name: name.to_owned(), members: Vec::new() });
        }
        ClientCommand::RemoveTeam { team } => teams.retain(|kept| kept.id != *team),
        ClientCommand::SetTeamMember { team, user, member } => {
            let Some(team) = teams.iter_mut().find(|kept| kept.id == *team) else { return Some(Vec::new()) };
            team.members.retain(|kept| kept != user);
            if *member {
                team.members.push(*user);
            }
        }
        _ => return None,
    }
    if let Err(error) = org.teams.store(&teams) {
        return refused(&format!("could not keep the teams: {error}"));
    }
    Some(org.to_readers(&ServerEvent::Teams(teams)))
}

static EDITING: Mutex<()> = Mutex::new(());
