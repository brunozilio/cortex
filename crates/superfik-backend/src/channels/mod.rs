//! Channel accounts (ADR 0012): the backend links each number or mailbox a
//! member connected on a channel plugin, keeps its conversations
//! ([`ChannelStore`]) and sends through a paced queue (`sender`).
//!
//! [`Channels::sync`] opens the links an owner's accounts need and closes
//! the rest. Each link reports on its own threads; those reports reach the
//! handler as [`Inbound`] through a queue (`set_inbound`), and
//! [`Channels::observe`] turns one into what the owner's clients should see
//! and, for a message the agent should answer, the `Incoming` that starts
//! the routines listening to the account.

pub mod mcp;
mod media;
mod outgoing;
mod report;
pub mod sender;
mod store;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crossbeam_channel::Sender;
pub use report::hook_path;
pub use store::{ChannelStore, FileChannelStore, InMemoryChannelStore};
use superfik_channels::{Channel, ChannelEvent, CloseReason, Events, LinkSpec, Opener};
use superfik_core::channel::{ChannelBook, ChannelKind, LinkStatus};
use superfik_core::routine::Incoming;
use superfik_core::{Connection, ConnectionId, Principal};
use superfik_protocol::ServerEvent;

use self::sender::{Clock, Outgoing, SystemClock};

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelAccount {
    pub connection: Connection,
    pub kind: ChannelKind,
    pub credentials: superfik_channels::Credentials,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    pub owner: Principal,
    pub connection: ConnectionId,
    pub event: ChannelEvent,
}

#[derive(Debug, Default)]
pub struct Observed {
    pub update: Option<ServerEvent>,
    pub incoming: Option<Incoming>,
}

struct Link {
    owner: Principal,
    connection: Connection,
    kind: ChannelKind,
    status: LinkStatus,
    channel: Option<Arc<dyn Channel>>,
    queue: Option<Sender<Outgoing>>,
}

struct Inner {
    openers: Vec<Arc<dyn Opener>>,
    root: PathBuf,
    store: Arc<dyn ChannelStore>,
    clock: Arc<dyn Clock>,
    books: Mutex<HashMap<(Principal, ConnectionId), ChannelBook>>,
    links: Mutex<HashMap<ConnectionId, Link>>,
    inbound: Mutex<Option<Sender<Inbound>>>,
}

#[derive(Clone)]
pub struct Channels {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Channels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channels").field("openers", &self.inner.openers.len()).finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Channels {
    pub fn new(
        openers: Vec<Arc<dyn Opener>>,
        root: PathBuf,
        store: Arc<dyn ChannelStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let inner = Inner {
            openers,
            root,
            store,
            clock,
            books: Mutex::default(),
            links: Mutex::default(),
            inbound: Mutex::default(),
        };
        Self { inner: Arc::new(inner) }
    }

    pub fn none() -> Self {
        Self::new(Vec::new(), std::env::temp_dir(), Arc::new(InMemoryChannelStore::default()), Arc::new(SystemClock))
    }

    pub fn set_inbound(&self, inbound: Sender<Inbound>) {
        *lock(&self.inner.inbound) = Some(inbound);
    }

    pub fn sync(&self, owner: &Principal, accounts: &[ChannelAccount]) {
        let gone: Vec<Link> = {
            let mut links = lock(&self.inner.links);
            let ids: Vec<ConnectionId> = links
                .iter()
                .filter(|(id, link)| &link.owner == owner && !accounts.iter().any(|a| a.connection.id == **id))
                .map(|(id, _)| *id)
                .collect();
            ids.iter().filter_map(|id| links.remove(id)).collect()
        };
        for link in gone {
            if let Some(channel) = link.channel {
                channel.unlink();
            }
        }
        for account in accounts {
            let connection = &account.connection;
            let fresh = {
                let mut links = lock(&self.inner.links);
                let fresh = !links.contains_key(&connection.id);
                links.entry(connection.id).or_insert_with(|| Link {
                    owner: owner.clone(),
                    connection: connection.clone(),
                    kind: account.kind,
                    status: LinkStatus::Opening,
                    channel: None,
                    queue: None,
                });
                fresh
            };
            if fresh {
                self.open(owner.clone(), account.clone());
            }
        }
    }

    fn open(&self, owner: Principal, account: ChannelAccount) {
        let (connection, kind) = (account.connection.id, account.kind);
        let this = self.clone();
        let spawned = std::thread::Builder::new().name("superfik-channel-open".into()).spawn(move || {
            let report = this.reporter(owner.clone(), connection);
            let Some(opener) = this.inner.openers.iter().find(|o| o.kind() == kind) else {
                report(ChannelEvent::Closed(CloseReason::Failed(format!("{kind:?} is not available here"))));
                return;
            };
            let data_dir = this.inner.root.join(connection.to_string());
            let spec = LinkSpec { connection, kind, data_dir, credentials: account.credentials };
            match opener.open(&spec, Arc::clone(&report)) {
                Ok(channel) => {
                    let since = this.inner.clock.now_ms() - 24 * 60 * 60 * 1_000;
                    let sent = this.book(&owner, connection).ledger(since);
                    let clock = Arc::clone(&this.inner.clock);
                    let sent_report = Arc::clone(&report);
                    let record: sender::Record = Box::new(move |message| {
                        sent_report(ChannelEvent::Message {
                            message,
                            delivery: superfik_core::channel::Delivery::Live,
                        });
                    });
                    let queue = sender::spawn(Arc::clone(&channel), clock, kind.limits(), sent, record);
                    if let Some(link) = lock(&this.inner.links).get_mut(&connection) {
                        link.channel = Some(channel);
                        link.queue = queue;
                    }
                }
                Err(error) => report(ChannelEvent::Closed(CloseReason::Failed(error.to_string()))),
            }
        });
        if let Err(error) = spawned {
            tracing::warn!(%error, "could not open a channel");
        }
    }

    fn reporter(&self, owner: Principal, connection: ConnectionId) -> Events {
        let inbound = lock(&self.inner.inbound).clone();
        Arc::new(move |event| {
            if let Some(inbound) = &inbound {
                let _ = inbound.send(Inbound { owner: owner.clone(), connection, event });
            }
        })
    }

    pub fn update(&self, owner: &Principal, connection: ConnectionId) -> ServerEvent {
        let status = lock(&self.inner.links).get(&connection).map(|l| l.status.clone()).unwrap_or_default();
        let drafts = self.book(owner, connection).drafts;
        ServerEvent::Channel { connection, status, drafts }
    }

    pub fn list(&self, owner: &Principal) -> Vec<ServerEvent> {
        let ids: Vec<ConnectionId> =
            lock(&self.inner.links).iter().filter(|(_, l)| &l.owner == owner).map(|(id, _)| *id).collect();
        ids.into_iter().map(|id| self.update(owner, id)).collect()
    }

    pub fn accounts(&self, owner: &Principal, wanted: &[Connection]) -> Vec<(ConnectionId, String)> {
        let links = lock(&self.inner.links);
        wanted
            .iter()
            .filter(|c| links.get(&c.id).is_some_and(|l| &l.owner == owner))
            .map(|c| (c.id, c.title()))
            .collect()
    }

    pub fn book(&self, owner: &Principal, connection: ConnectionId) -> ChannelBook {
        let mut books = lock(&self.inner.books);
        books.entry((owner.clone(), connection)).or_insert_with(|| self.inner.store.load(owner, connection)).clone()
    }

    fn change(
        &self,
        owner: &Principal,
        connection: ConnectionId,
        rule: impl FnOnce(&mut ChannelBook) -> bool,
    ) -> (bool, Option<i64>) {
        let mut books = lock(&self.inner.books);
        let book = books.entry((owner.clone(), connection)).or_insert_with(|| self.inner.store.load(owner, connection));
        let last_seen = book.threads.iter().filter_map(|t| t.exchange.last_at).max();
        let changed = rule(book);
        if changed && let Err(error) = self.inner.store.save(owner, connection, book) {
            tracing::warn!(%error, "could not save a channel's conversations");
        }
        (changed, last_seen)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_official;
