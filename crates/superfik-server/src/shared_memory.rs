//! Memory members share with a project or the whole organization (ADR
//! 0004), kept by the organization's server. Each scope numbers its own
//! memories. A change goes to everyone as the project's new list.

use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::memory::shared::{self, SharedMemory};
use superfik_core::project::ProjectRef;
use superfik_core::sharing::{MemoryScope, Viewer};
use superfik_core::{Member, Principal, delegation};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent};

use crate::Organization;

pub trait SharedMemories: Send + Sync {
    fn load(&self, project: Option<&ProjectRef>) -> Vec<SharedMemory>;
    fn store(&self, project: Option<&ProjectRef>, memories: &[SharedMemory]) -> std::io::Result<()>;
}

#[derive(Debug)]
pub struct FileSharedMemories {
    root: PathBuf,
    writing: Mutex<()>,
}

impl FileSharedMemories {
    pub fn new(root: PathBuf) -> Self {
        Self { root, writing: Mutex::new(()) }
    }

    fn file(&self, project: Option<&ProjectRef>) -> PathBuf {
        match project {
            Some(project) => self.root.join(format!("{}.json", crate::project_sessions::folder_name(project))),
            None => self.root.join("organization.json"),
        }
    }
}

impl SharedMemories for FileSharedMemories {
    fn load(&self, project: Option<&ProjectRef>) -> Vec<SharedMemory> {
        std::fs::read(self.file(project)).ok().and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default()
    }

    fn store(&self, project: Option<&ProjectRef>, memories: &[SharedMemory]) -> std::io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        std::fs::create_dir_all(&self.root)?;
        let file = self.file(project);
        let temporary = file.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(memories).map_err(std::io::Error::other)?)?;
        std::fs::rename(temporary, file)
    }
}

#[derive(Debug, Default)]
pub struct InMemorySharedMemories(Mutex<Vec<(Option<ProjectRef>, Vec<SharedMemory>)>>);

impl SharedMemories for InMemorySharedMemories {
    fn load(&self, project: Option<&ProjectRef>) -> Vec<SharedMemory> {
        let kept = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.iter().find(|(p, _)| p.as_ref() == project).map(|(_, m)| m.clone()).unwrap_or_default()
    }

    fn store(&self, project: Option<&ProjectRef>, memories: &[SharedMemory]) -> std::io::Result<()> {
        let mut kept = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.retain(|(p, _)| p.as_ref() != project);
        kept.push((project.cloned(), memories.to_vec()));
        Ok(())
    }
}

fn listed(store: &dyn SharedMemories, project: &ProjectRef) -> Vec<SharedMemory> {
    let mut all = store.load(Some(project));
    all.extend(store.load(None));
    all
}

fn place(scope: MemoryScope, project: &ProjectRef) -> Option<Option<&ProjectRef>> {
    match scope {
        MemoryScope::Project => Some(Some(project)),
        MemoryScope::Organization => Some(None),
        MemoryScope::Personal | MemoryScope::Team(_) => None,
    }
}

pub fn answer(
    org: &Organization,
    store: &dyn SharedMemories,
    by: &Principal,
    command: &ClientCommand,
) -> Option<Vec<Addressed>> {
    let to_sender = |event| Addressed { to: Recipient::Principal(by.clone()), event };
    let refused = |why: &str| Some(vec![to_sender(ServerEvent::error(why))]);
    let project = match command {
        ClientCommand::LoadSharedMemory { project }
        | ClientCommand::ShareMemory { project, .. }
        | ClientCommand::UnshareMemory { project, .. } => project,
        _ => return None,
    };
    let Principal::Member(member) = by else { return refused("not allowed") };
    let Some(role) = org.directory.role(member).filter(|_| org.reads(member)) else { return refused("not allowed") };
    let viewer = Viewer { user: member.user, teams: &[], org: role, project: None };
    let _editing = EDITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let changed = match command {
        ClientCommand::ShareMemory { scope, memory, .. } => {
            let Some(at) = place(*scope, project) else { return refused("this memory cannot be shared there") };
            if !scope.writable(member.user, &viewer) {
                return refused("not allowed");
            }
            let mut memories = store.load(at);
            let memory = superfik_core::Memory { id: shared::next_id(&memories), ..memory.clone() };
            let author_name = name_of(org, member).unwrap_or_default();
            memories.push(SharedMemory { scope: *scope, memory, author: member.user, author_name });
            store.store(at, &memories)
        }
        ClientCommand::UnshareMemory { scope, id, .. } => {
            let Some(at) = place(*scope, project) else { return Some(Vec::new()) };
            let mut memories = store.load(at);
            let Some(index) = memories.iter().position(|m| m.memory.id == *id) else { return Some(Vec::new()) };
            if memories[index].author != member.user && !delegation::approves(role) {
                return refused("not allowed");
            }
            memories.remove(index);
            store.store(at, &memories)
        }
        _ => {
            let memories = listed(store, project);
            return Some(vec![to_sender(ServerEvent::SharedMemory { project: project.clone(), memories })]);
        }
    };
    if let Err(error) = changed {
        return refused(&format!("could not keep the shared memory: {error}"));
    }
    let memories = listed(store, project);
    Some(org.to_readers(&ServerEvent::SharedMemory { project: project.clone(), memories }))
}

static EDITING: Mutex<()> = Mutex::new(());

fn name_of(org: &Organization, member: &Member) -> Option<String> {
    let accounts = org.directory.members().ok()?;
    accounts.into_iter().find(|account| account.user == member.user).map(|account| account.name)
}
