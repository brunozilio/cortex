//! Sessions an organization's server keeps by project (ADR 0004): each
//! member's own, and those others shared, as their visibility allows.
//! `LoadSessions`/`SaveSession` naming a project are answered here; a
//! save that others may see sends them the project's shared list again.

use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::project::ProjectRef;
use superfik_core::sharing::Viewer;
use superfik_core::{Member, Principal, SessionRecord, UserId};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent, SharedSession};

use crate::Organization;

pub trait ProjectSessions: Send + Sync {
    fn all(&self, project: &ProjectRef) -> Vec<(UserId, SessionRecord)>;
    fn save(&self, project: &ProjectRef, owner: UserId, session: &SessionRecord) -> std::io::Result<()>;
}

#[derive(Debug)]
pub struct FileProjectSessions {
    root: PathBuf,
    writing: Mutex<()>,
}

impl FileProjectSessions {
    pub fn new(root: PathBuf) -> Self {
        Self { root, writing: Mutex::new(()) }
    }

    fn dir(&self, project: &ProjectRef) -> PathBuf {
        self.root.join(folder_name(project))
    }
}

pub(crate) fn folder_name(project: &ProjectRef) -> String {
    let name = match project {
        ProjectRef::Remote(remote) => format!("remote-{remote}"),
        ProjectRef::Local(path) => format!("local-{}", path.display()),
    };
    name.chars().map(|c| if c.is_alphanumeric() || c == '.' { c } else { '-' }).collect()
}

impl ProjectSessions for FileProjectSessions {
    fn all(&self, project: &ProjectRef) -> Vec<(UserId, SessionRecord)> {
        let Ok(members) = std::fs::read_dir(self.dir(project)) else { return Vec::new() };
        members
            .filter_map(Result::ok)
            .filter_map(|member| {
                let owner = UserId::from_uuid(member.file_name().to_str()?.parse().ok()?);
                Some(std::fs::read_dir(member.path()).ok()?.filter_map(Result::ok).filter_map(move |file| {
                    let session = serde_json::from_slice(&std::fs::read(file.path()).ok()?).ok()?;
                    Some((owner, session))
                }))
            })
            .flatten()
            .collect()
    }

    fn save(&self, project: &ProjectRef, owner: UserId, session: &SessionRecord) -> std::io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = self.dir(project).join(owner.to_string());
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_vec_pretty(session).map_err(std::io::Error::other)?;
        let temporary = dir.join(format!("{}.tmp", session.id));
        std::fs::write(&temporary, json)?;
        std::fs::rename(temporary, dir.join(format!("{}.json", session.id)))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryProjectSessions(Mutex<Vec<(ProjectRef, UserId, SessionRecord)>>);

impl ProjectSessions for InMemoryProjectSessions {
    fn all(&self, project: &ProjectRef) -> Vec<(UserId, SessionRecord)> {
        let kept = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.iter().filter(|(p, ..)| p == project).map(|(_, owner, session)| (*owner, session.clone())).collect()
    }

    fn save(&self, project: &ProjectRef, owner: UserId, session: &SessionRecord) -> std::io::Result<()> {
        let mut kept = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.retain(|(p, o, s)| !(p == project && *o == owner && s.id == session.id));
        kept.push((project.clone(), owner, session.clone()));
        Ok(())
    }
}

pub fn answer(
    org: &Organization,
    store: &dyn ProjectSessions,
    by: &Principal,
    command: &ClientCommand,
) -> Option<Vec<Addressed>> {
    let to_sender = |event| Addressed { to: Recipient::Principal(by.clone()), event };
    let Principal::Member(member) = by else { return None };
    match command {
        ClientCommand::LoadSessions { path, project: Some(project) } => {
            let all = store.all(project);
            let mine = all.iter().filter(|(owner, _)| *owner == member.user).map(|(_, s)| s.clone()).collect();
            Some(vec![
                to_sender(ServerEvent::Sessions { path: path.clone(), sessions: mine }),
                to_sender(ServerEvent::SharedSessions {
                    project: project.clone(),
                    sessions: shared_with(org, &all, member),
                }),
            ])
        }
        ClientCommand::SaveSession { project: Some(project), session, .. } => {
            if let Err(error) = store.save(project, member.user, session) {
                return Some(vec![to_sender(ServerEvent::error(format!("could not keep the session: {error}")))]);
            }
            let all = store.all(project);
            let others = org.directory.members().unwrap_or_default();
            Some(
                others
                    .into_iter()
                    .filter(|account| account.user != member.user)
                    .map(|account| {
                        let colleague = Member { user: account.user, org: member.org };
                        Addressed {
                            to: Recipient::Principal(Principal::Member(colleague)),
                            event: ServerEvent::SharedSessions {
                                project: project.clone(),
                                sessions: shared_with(org, &all, &colleague),
                            },
                        }
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

pub(crate) fn sees(org: &Organization, member: &Member, owner: UserId, session: &SessionRecord) -> bool {
    let Some(role) = org.directory.role(member) else { return false };
    let teams = org.teams_of(member);
    session.visibility.shows(owner, &Viewer { user: member.user, teams: &teams, org: role, project: None })
}

fn shared_with(org: &Organization, all: &[(UserId, SessionRecord)], member: &Member) -> Vec<SharedSession> {
    let Some(role) = org.directory.role(member) else { return Vec::new() };
    let teams = org.teams_of(member);
    let viewer = Viewer { user: member.user, teams: &teams, org: role, project: None };
    let names = org.directory.members().unwrap_or_default();
    all.iter()
        .filter(|(owner, session)| *owner != member.user && session.visibility.shows(*owner, &viewer))
        .map(|(owner, session)| SharedSession {
            owner: *owner,
            owner_name: names.iter().find(|account| account.user == *owner).map(|a| a.name.clone()).unwrap_or_default(),
            session: session.clone(),
        })
        .collect()
}
