//! A hosted server audits every command it receives, with who sent it and
//! whether their role allowed it.

use std::sync::Arc;

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories, RoleAuthorizer};
use superfik_core::Principal;
use superfik_core::access::OrgRole;
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope};
use superfik_server::audit::{Entry, MemoryAudit, Outcome};
use superfik_server::testing::{self, admit};
use superfik_server::{DirectoryRoles, Hosted, Server};

fn audited(role: OrgRole, commands: Vec<ClientCommand>) -> (Principal, Vec<Entry>) {
    let organization = Arc::new(testing::organization(Arc::new(|| 42)));
    let (member, _) = admit(&organization, "Olga", role);
    let audit = Arc::new(MemoryAudit::default());
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let roles = DirectoryRoles(Arc::clone(&organization));
    services.authorizer = Arc::new(RoleAuthorizer::new(organization.directory.org(), roles));
    let server = Server::start_for(services, Some(Hosted { audit: audit.clone(), ..Hosted::in_memory(organization) }))
        .expect("server");
    let connection = server.connect(Principal::Member(member), Arc::new(|| {}));
    for command in commands {
        connection.send(Envelope::local(command)).expect("send");
    }
    let entries = audit.0.lock().expect("lock").clone();
    (Principal::Member(member), entries)
}

#[test]
fn every_command_is_audited_with_its_sender() {
    let (owner, entries) = audited(OrgRole::Owner, vec![ClientCommand::ListModels, ClientCommand::ListMembers]);
    let seen: Vec<(&str, u64)> = entries.iter().map(|entry| (entry.command.as_str(), entry.at_unix)).collect();
    assert_eq!(seen, [("list_models", 42), ("list_members", 42)]);
    assert!(entries.iter().all(|entry| entry.principal == owner));
    assert!(entries.iter().all(|entry| entry.outcome == Some(Outcome::Allowed)));
}

#[test]
fn a_refused_command_is_audited_as_refused() {
    let run = superfik_core::RunId::new();
    let (_, entries) = audited(OrgRole::Guest, vec![ClientCommand::CancelRun { run }]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Some(Outcome::Refused), "a guest runs nothing");
}
