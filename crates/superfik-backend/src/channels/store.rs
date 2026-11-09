//! Where channel books are kept: one JSON file per owner and account, or
//! in memory for tests.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::channel::ChannelBook;
use superfik_core::{ConnectionId, Principal};

use crate::memory::{app_data_dir, owner_key};

pub trait ChannelStore: Send + Sync {
    fn load(&self, owner: &Principal, connection: ConnectionId) -> ChannelBook;
    fn save(&self, owner: &Principal, connection: ConnectionId, book: &ChannelBook) -> std::io::Result<()>;
}

#[derive(Debug, Clone)]
pub struct FileChannelStore {
    root: PathBuf,
}

impl FileChannelStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("channels"))
    }

    fn path(&self, owner: &Principal, connection: ConnectionId) -> PathBuf {
        self.root.join(owner_key(owner)).join(format!("{connection}.json"))
    }
}

impl ChannelStore for FileChannelStore {
    fn load(&self, owner: &Principal, connection: ConnectionId) -> ChannelBook {
        std::fs::read(self.path(owner, connection))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, owner: &Principal, connection: ConnectionId, book: &ChannelBook) -> std::io::Result<()> {
        let path = self.path(owner, connection);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec(book).map_err(std::io::Error::other)?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json)?;
        std::fs::rename(partial, path)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryChannelStore {
    books: Mutex<HashMap<(Principal, ConnectionId), ChannelBook>>,
}

impl ChannelStore for InMemoryChannelStore {
    fn load(&self, owner: &Principal, connection: ConnectionId) -> ChannelBook {
        let books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.get(&(owner.clone(), connection)).cloned().unwrap_or_default()
    }

    fn save(&self, owner: &Principal, connection: ConnectionId, book: &ChannelBook) -> std::io::Result<()> {
        let mut books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.insert((owner.clone(), connection), book.clone());
        Ok(())
    }
}
