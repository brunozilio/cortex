//! Ana (member) hands what her agent asks to the approvers; Olga (owner)
//! sees it and allows it, and Ana's app hears who did. A member who does
//! not approve can neither see nor answer it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::{AgentAsk, AskId, AskKind, Decision, Member, Principal, RunId};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Server};

const NOW: u64 = 1_800_000_000;

fn team() -> (Server, Member, Member, Member) {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let olga = admit(&organization, "Olga", OrgRole::Owner).0;
    let (ana, rui) = (admit(&organization, "Ana", OrgRole::Member).0, admit(&organization, "Rui", OrgRole::Member).0);
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let hosted = Hosted::in_memory(organization);
    (Server::start_for(services, Some(hosted)).expect("server"), olga, ana, rui)
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

fn deploy() -> AgentAsk {
    let kind = AskKind::Permission { tool: "Bash".into(), subject: "cargo publish".into(), rememberable: false };
    AgentAsk { id: AskId("p1".into()), kind }
}

#[test]
fn an_approver_allows_what_a_member_handed_over() {
    let (server, olga, ana, _) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    let run = RunId::new();
    send(&ana, ClientCommand::DelegateAsk { run, ask: deploy(), session: Some("Release".into()) });
    let ServerEvent::Delegated(waiting) = next(&olga) else { panic!("not listed") };
    assert_eq!((waiting.len(), waiting[0].requester_name.as_str()), (1, "ana"));
    assert_eq!(waiting[0].session.as_deref(), Some("Release"));
    send(&olga, ClientCommand::AnswerDelegated { run, ask: AskId("p1".into()), decision: Decision::Allow });
    assert_eq!(next(&olga), ServerEvent::Delegated(vec![]), "no longer waiting");
    let answered = next(&ana);
    let expected =
        ServerEvent::DelegatedAnswered { run, ask: AskId("p1".into()), decision: Decision::Allow, by: "olga".into() };
    assert_eq!(answered, expected);
}

#[test]
fn a_member_who_does_not_approve_neither_sees_nor_answers() {
    let (server, olga, ana, rui) = team();
    let (_olga, ana, rui) = (connect(&server, olga), connect(&server, ana), connect(&server, rui));
    let run = RunId::new();
    send(&ana, ClientCommand::DelegateAsk { run, ask: deploy(), session: None });
    send(&rui, ClientCommand::ListDelegated);
    assert_eq!(next(&rui), ServerEvent::Delegated(vec![]));
    send(&rui, ClientCommand::AnswerDelegated { run, ask: AskId("p1".into()), decision: Decision::Allow });
    assert!(matches!(next(&rui), ServerEvent::Error { .. }));
    assert!(ana.try_recv().is_none(), "nobody answered Ana");
}

#[test]
fn the_requester_takes_an_ask_back() {
    let (server, olga, ana, _) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    let run = RunId::new();
    send(&ana, ClientCommand::DelegateAsk { run, ask: deploy(), session: None });
    let _ = next(&olga);
    send(&ana, ClientCommand::AnswerDelegated { run, ask: AskId("p1".into()), decision: Decision::Deny });
    assert_eq!(next(&olga), ServerEvent::Delegated(vec![]));
    assert!(matches!(next(&ana), ServerEvent::DelegatedAnswered { decision: Decision::Deny, .. }));
}
