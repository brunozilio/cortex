//! The server: workers over one command handler, and the connections its
//! events fan out to.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender};
use superfik_backend::{Authorizer, Backend, BackendError, LocalServices, Outbox, Wake, Workers, forbidden};
use superfik_core::Principal;
use superfik_protocol::{Addressed, ClientCommand, Envelope, Recipient, ServerEvent};

use crate::presence::Presence;

use crate::audit::{AuditLog, Entry, Outcome};
use crate::checkouts::Checkouts;
use crate::delegations::{self, Delegations};
use crate::governance::{self, Governance};

#[path = "member_commands.rs"]
mod member_commands;
#[path = "server_hooks.rs"]
mod server_hooks;
use crate::hub::{ConnectionId, Hub};
use crate::library::{self, LibraryStore};
use crate::project_sessions::{self, ProjectSessions};
use crate::shared_memory::{self, SharedMemories};
use crate::{Organization, members};

#[derive(Clone)]
pub struct Server {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server").finish_non_exhaustive()
    }
}

struct Inner {
    workers: Workers,
    authorizer: Arc<dyn Authorizer>,
    links: Arc<Links>,
    organization: Option<Hosted>,
    delegations: Delegations,
    presence: Presence,
    preparing: std::sync::atomic::AtomicUsize,
}

#[derive(Clone)]
pub struct Hosted {
    pub organization: Arc<Organization>,
    pub audit: Arc<dyn AuditLog>,
    pub sessions: Arc<dyn ProjectSessions>,
    pub memory: Arc<dyn SharedMemories>,
    pub library: Arc<dyn LibraryStore>,
    pub governance: Arc<dyn Governance>,
    pub checkouts: Arc<dyn Checkouts>,
    pub comments: Arc<dyn crate::comments::CommentStore>,
    pub github: GitHubSignIn,
}

pub use crate::github_login::GitHubSignIn;

impl Hosted {
    pub fn in_memory(organization: Arc<Organization>) -> Self {
        Self {
            organization,
            audit: Arc::new(crate::audit::MemoryAudit::default()),
            sessions: Arc::new(crate::project_sessions::InMemoryProjectSessions::default()),
            memory: Arc::new(crate::shared_memory::InMemorySharedMemories::default()),
            library: Arc::new(crate::library::InMemoryLibrary::default()),
            governance: Arc::new(crate::governance::InMemoryGovernance::default()),
            checkouts: Arc::new(crate::checkouts::GitCheckouts::new(crate::checkouts::scratch_root())),
            comments: Arc::new(crate::comments::InMemoryComments::default()),
            github: GitHubSignIn::in_memory(None),
        }
    }
}

impl std::fmt::Debug for Hosted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hosted").field("organization", &self.organization).finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Links {
    hub: Mutex<Hub>,
    clients: Mutex<HashMap<ConnectionId, Client>>,
}

struct Client {
    events: Sender<ServerEvent>,
    wake: Wake,
}

impl Server {
    pub fn start(services: LocalServices) -> Result<Self, BackendError> {
        Self::start_for(services, None)
    }

    pub fn start_for(mut services: LocalServices, organization: Option<Hosted>) -> Result<Self, BackendError> {
        let (sender, addressed) = crossbeam_channel::unbounded();
        let links = Arc::new(Links::default());
        if let Some(hosted) = &organization {
            let to = Arc::downgrade(&links);
            services.budget = Arc::new(crate::routine_budget::GovernedBudget {
                org: Arc::clone(&hosted.organization),
                store: Arc::clone(&hosted.governance),
                deliver: Arc::new(move |addressed| {
                    if let Some(links) = to.upgrade() {
                        links.deliver(addressed);
                    }
                }),
            });
        }
        let authorizer = Arc::clone(&services.authorizer);
        let workers = Workers::start(services, Outbox::addressed(sender, Arc::new(|| {})))?;
        let dispatcher = Arc::clone(&links);
        std::thread::Builder::new()
            .name("superfik-server-dispatch".to_owned())
            .spawn(move || {
                for addressed in addressed {
                    dispatcher.deliver(addressed);
                }
            })
            .map_err(BackendError::Worker)?;
        let delegations = Delegations::default();
        let presence = Presence::default();
        let preparing = std::sync::atomic::AtomicUsize::new(0);
        let inner = Inner { workers, authorizer, links, organization, delegations, presence, preparing };
        Ok(Self { inner: Arc::new(inner) })
    }

    pub fn connect(&self, principal: Principal, wake: Wake) -> Connection {
        let (events_tx, events) = crossbeam_channel::unbounded();
        let id = lock(&self.inner.links.hub).join(principal.clone());
        lock(&self.inner.links.clients).insert(id, Client { events: events_tx, wake });
        Connection { id, principal, events, server: Arc::clone(&self.inner) }
    }
}

impl Links {
    fn deliver(&self, Addressed { to, event }: Addressed) {
        let reached = {
            let mut hub = lock(&self.hub);
            let reached = hub.reach(&to);
            if let ServerEvent::Run { run, event } = &event
                && event.is_terminal()
            {
                hub.finished(*run);
            }
            reached
        };
        let clients = lock(&self.clients);
        for client in reached.iter().filter_map(|id| clients.get(id)) {
            if client.events.send(event.clone()).is_ok() {
                (client.wake)();
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub struct Connection {
    id: ConnectionId,
    principal: Principal,
    events: Receiver<ServerEvent>,
    server: Arc<Inner>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection").field("id", &self.id).field("principal", &self.principal).finish_non_exhaustive()
    }
}

impl Backend for Connection {
    fn send(&self, command: Envelope<ClientCommand>) -> Result<(), BackendError> {
        let hosted = self.server.organization.as_ref();
        if let Some(hosted) = hosted {
            let admitted = self.admitted(hosted, &command.payload);
            let authorized = self.server.authorizer.authorize(&self.principal, &command.payload);
            let outcome = if admitted.is_ok() && authorized.is_ok() { Outcome::Allowed } else { Outcome::Refused };
            hosted.audit.record(&Entry::of(&self.principal, &command.payload, hosted.organization.now(), outcome));
            if let Err(error) = admitted {
                let event = ServerEvent::error(error.to_string());
                self.server.links.deliver(Addressed { to: Recipient::Principal(self.principal.clone()), event });
                return Ok(());
            }
        }
        if let (ClientCommand::Viewing { session }, Some(hosted), Principal::Member(member)) =
            (&command.payload, hosted, &self.principal)
        {
            let name = hosted.organization.directory.members().ok().and_then(|accounts| {
                accounts.into_iter().find(|account| account.user == member.user).map(|account| account.name)
            });
            if let Some(list) = self.server.presence.set(self.id, member.user, name.unwrap_or_default(), *session) {
                for addressed in hosted.organization.to_readers(&ServerEvent::Presence(list)) {
                    self.server.links.deliver(addressed);
                }
            }
            return Ok(());
        }
        if let Some(handled) = hosted.and_then(|hosted| self.member_command(hosted, &command.payload)) {
            return handled;
        }
        let answered = hosted.and_then(|hosted| {
            let (org, by, command) = (&hosted.organization, &self.principal, &command.payload);
            members::answer(org, by, command)
                .or_else(|| project_sessions::answer(org, hosted.sessions.as_ref(), by, command))
                .or_else(|| delegations::answer(org, &self.server.delegations, by, command))
                .or_else(|| shared_memory::answer(org, hosted.memory.as_ref(), by, command))
                .or_else(|| library::answer(org, hosted.library.as_ref(), by, command))
                .or_else(|| crate::teams::answer(org, by, command))
                .or_else(|| {
                    let (sessions, comments) = (hosted.sessions.as_ref(), hosted.comments.as_ref());
                    crate::comments::answer(org, sessions, comments, by, command)
                })
                .or_else(|| governance::answer(org, hosted.governance.as_ref(), by, command))
        });
        if let Some(events) = answered {
            for event in events {
                self.server.links.deliver(event);
            }
            return Ok(());
        }
        if !lock(&self.server.links.hub).heard(self.id, &command.payload) {
            let event = ServerEvent::error(forbidden(&self.principal, command.payload.name()).to_string());
            self.server.links.deliver(Addressed { to: Recipient::Principal(self.principal.clone()), event });
            return Ok(());
        }
        self.server.workers.send(Envelope::new(self.principal.clone(), command.payload))
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        lock(&self.server.links.hub).leave(self.id);
        lock(&self.server.links.clients).remove(&self.id);
        if let (Some(list), Some(hosted)) = (self.server.presence.leave(self.id), &self.server.organization) {
            for addressed in hosted.organization.to_readers(&ServerEvent::Presence(list)) {
                self.server.links.deliver(addressed);
            }
        }
    }
}
