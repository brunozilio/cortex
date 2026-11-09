//! The organization's library (`superfik_core::library`), kept by its
//! server: owners and admins publish and remove; every member lists it,
//! and every change goes to everyone.

use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::Principal;
use superfik_core::library::{self, LibraryItem};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent};

use crate::Organization;

pub trait LibraryStore: Send + Sync {
    fn load(&self) -> Vec<LibraryItem>;
    fn store(&self, items: &[LibraryItem]) -> std::io::Result<()>;
}

#[derive(Debug)]
pub struct FileLibrary {
    file: PathBuf,
    writing: Mutex<()>,
}

impl FileLibrary {
    pub fn new(file: PathBuf) -> Self {
        Self { file, writing: Mutex::new(()) }
    }
}

impl LibraryStore for FileLibrary {
    fn load(&self) -> Vec<LibraryItem> {
        std::fs::read(&self.file).ok().and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default()
    }

    fn store(&self, items: &[LibraryItem]) -> std::io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = self.file.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(items).map_err(std::io::Error::other)?)?;
        std::fs::rename(temporary, &self.file)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryLibrary(Mutex<Vec<LibraryItem>>);

impl LibraryStore for InMemoryLibrary {
    fn load(&self) -> Vec<LibraryItem> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn store(&self, items: &[LibraryItem]) -> std::io::Result<()> {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = items.to_vec();
        Ok(())
    }
}

pub fn answer(
    org: &Organization,
    store: &dyn LibraryStore,
    by: &Principal,
    command: &ClientCommand,
) -> Option<Vec<Addressed>> {
    let to_sender = |event| Addressed { to: Recipient::Principal(by.clone()), event };
    let refused = |why: &str| Some(vec![to_sender(ServerEvent::error(why))]);
    if !matches!(
        command,
        ClientCommand::ListLibrary | ClientCommand::PublishToLibrary { .. } | ClientCommand::RemoveFromLibrary { .. }
    ) {
        return None;
    }
    let Principal::Member(member) = by else { return refused("not allowed") };
    let Some(role) = org.directory.role(member).filter(|_| org.reads(member)) else { return refused("not allowed") };
    let _editing = EDITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut items = store.load();
    match command {
        ClientCommand::PublishToLibrary { item } => {
            if !superfik_core::access::administers(role) {
                return refused("only owners and admins publish to the library");
            }
            if !library::valid_name(&item.name) {
                return refused("library names are lower-case letters, digits and dashes");
            }
            let author_name = org
                .directory
                .members()
                .ok()
                .and_then(|accounts| accounts.into_iter().find(|a| a.user == member.user))
                .map(|account| account.name)
                .unwrap_or_default();
            library::upsert(&mut items, LibraryItem { author_name, ..item.clone() });
        }
        ClientCommand::RemoveFromLibrary { kind, name } => {
            if !superfik_core::access::administers(role) {
                return refused("only owners and admins change the library");
            }
            items.retain(|item| !(item.kind == *kind && &item.name == name));
        }
        _ => return Some(vec![to_sender(ServerEvent::Library(items))]),
    }
    if let Err(error) = store.store(&items) {
        return refused(&format!("could not keep the library: {error}"));
    }
    Some(org.to_readers(&ServerEvent::Library(items)))
}

static EDITING: Mutex<()> = Mutex::new(());
