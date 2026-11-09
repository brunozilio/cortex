//! Olga (owner) sets the policy and sees what everyone spent; Ana (member)
//! reports her runs, hears her own total and cannot change the policy.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::policy::Policy;
use superfik_core::{Member, PermissionMode, Principal, RunId};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Server};

const NOW: u64 = 1_792_022_400;

fn team() -> (Server, Member, Member) {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let (olga, _) = admit(&organization, "Olga", OrgRole::Owner);
    let (ana, _) = admit(&organization, "Ana", OrgRole::Member);
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    (Server::start_for(services, Some(Hosted::in_memory(organization))).expect("server"), olga, ana)
}

fn connect(server: &Server, member: Member) -> Connection {
    server.connect(Principal::Member(member), Arc::new(|| {}))
}

fn send(connection: &Connection, command: ClientCommand) {
    connection.send(Envelope::local(command)).expect("send");
}

fn next(connection: &Connection) -> ServerEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(event) = connection.try_recv() {
            return event;
        }
        assert!(Instant::now() < deadline, "nothing arrived");
        std::thread::yield_now();
    }
}

#[test]
fn an_owner_sets_the_policy_for_everyone() {
    let (server, olga, ana) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    let policy = Policy { max_mode: Some(PermissionMode::AcceptEdits), ..Policy::default() };
    send(&ana, ClientCommand::SetPolicy { policy: policy.clone() });
    assert!(matches!(next(&ana), ServerEvent::Error { .. }), "members cannot");
    send(&olga, ClientCommand::SetPolicy { policy: policy.clone() });
    assert_eq!(next(&ana), ServerEvent::Policy(policy.clone()));
    send(&ana, ClientCommand::GetPolicy);
    assert_eq!(next(&ana), ServerEvent::Policy(policy));
    assert_eq!(next(&ana), ServerEvent::Spent { month: "2026-10".into(), usd: 0.0 });
}

#[test]
fn spending_adds_up_per_member_and_month() {
    let (server, olga, ana) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    for cost_usd in [1.25, 0.75] {
        send(&ana, ClientCommand::ReportUsage { run: RunId::new(), model: "claude-opus-5-5".into(), cost_usd });
    }
    assert_eq!(next(&ana), ServerEvent::Spent { month: "2026-10".into(), usd: 1.25 });
    assert_eq!(next(&ana), ServerEvent::Spent { month: "2026-10".into(), usd: 2.0 });
    send(&ana, ClientCommand::ListUsage);
    assert!(matches!(next(&ana), ServerEvent::Error { .. }), "members see only their own");
    send(&olga, ClientCommand::ListUsage);
    let ServerEvent::Usage(usage) = next(&olga) else { panic!("no usage") };
    let lines: Vec<(&str, f64, u32)> = usage.iter().map(|u| (u.name.as_str(), u.cost_usd, u.runs)).collect();
    assert!(lines.contains(&("ana", 2.0, 2)) && lines.contains(&("olga", 0.0, 0)));
}
