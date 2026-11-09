//! Settings › Server scenarios: connecting to a team's server, and for an
//! owner its domain, users, an ask waiting for approval, the library, the
//! policy and this month's spending.

use std::path::PathBuf;

use superfik_core::access::{Colleague, OrgRole};
use superfik_core::delegation::DelegatedAsk;
use superfik_core::library::{LibraryItem, LibraryKind};
use superfik_core::policy::Policy;
use superfik_core::{
    AgentAsk, AskId, AskKind, EngineKind, Member, ModelOption, OrgId, PermissionMode, Principal, RunId, UserId,
};
use superfik_protocol::{MemberUsage, PROTOCOL_VERSION, ServerEvent};

use super::scenarios::SyntheticImage;
use crate::state::{Action, Session, SettingsPane};

pub fn governance() -> (Session, Vec<SyntheticImage>) {
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    let server = |s: &mut Session, event| {
        s.apply(Action::Server(event), 0.0);
    };
    let me = Member { user: UserId::new(), org: OrgId::new() };
    server(&mut s, ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) });
    let ana = UserId::new();
    let colleague = |user, name: &str, role| Colleague { user, name: name.into(), role, pending: false };
    server(
        &mut s,
        ServerEvent::Members(vec![colleague(ana, "ana", OrgRole::Member), colleague(me.user, "olga", OrgRole::Owner)]),
    );
    let settings = superfik_protocol::ServerSettings { domain: Some("superfik.acme.io".into()) };
    server(&mut s, ServerEvent::ServerSettings(settings));
    server(
        &mut s,
        ServerEvent::Models(vec![
            ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
            ModelOption::new(EngineKind::Native, "claude-sonnet-5-5", "Sonnet 5.5"),
        ]),
    );
    let kind =
        AskKind::Permission { tool: "Bash".into(), subject: "cargo publish -p rocket".into(), rememberable: false };
    server(
        &mut s,
        ServerEvent::Delegated(vec![DelegatedAsk {
            run: RunId::new(),
            ask: AgentAsk { id: AskId("p1".into()), kind },
            requester: ana,
            requester_name: "Ana".into(),
            session: Some("Release 2.4".into()),
        }]),
    );
    let item =
        |kind, name: &str| LibraryItem { kind, name: name.into(), body: String::new(), author_name: "Olga".into() };
    server(
        &mut s,
        ServerEvent::Library(vec![item(LibraryKind::Skill, "release-train"), item(LibraryKind::Agent, "reviewer")]),
    );
    server(
        &mut s,
        ServerEvent::Policy(Policy {
            models: Some(vec!["claude-sonnet-5-5".into()]),
            max_mode: Some(PermissionMode::AcceptEdits),
            monthly_budget_usd: Some(100.0),
        }),
    );
    let spent = |user, name: &str, cost_usd, runs| MemberUsage {
        user,
        name: name.into(),
        month: "2026-10".into(),
        cost_usd,
        runs,
    };
    server(&mut s, ServerEvent::Usage(vec![spent(ana, "ana", 42.3, 57), spent(me.user, "olga", 8.75, 12)]));
    let design =
        superfik_core::sharing::Team { id: superfik_core::TeamId::new(), name: "Design".into(), members: vec![ana] };
    let platform =
        superfik_core::sharing::Team { id: superfik_core::TeamId::new(), name: "Platform".into(), members: vec![] };
    server(&mut s, ServerEvent::Teams(vec![design.clone(), platform]));
    s.apply(Action::ChooseSetting(crate::state::Setting::Team(crate::state::TeamChoice::Open(Some(design.id)))), 0.0);
    s.apply(Action::OpenSettings, 0.0);
    s.apply(Action::ShowSettingsPane(SettingsPane::Server), 0.0);
    s.notice = None;
    (s, Vec::new())
}

pub fn connect(first_access: bool) -> (Session, Vec<SyntheticImage>) {
    use crate::state::{EditKey, ServerAction, ServerField};
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.apply(Action::OpenSettings, 0.0);
    s.apply(Action::ShowSettingsPane(SettingsPane::Server), 0.0);
    if first_access {
        s.apply(Action::ServerLink(ServerAction::ToggleFirstAccess), 0.0);
    }
    let mut write = |field, text: &str| {
        s.apply(Action::ServerLink(ServerAction::Focus(field)), 0.0);
        s.apply(Action::ServerLink(ServerAction::Edit(EditKey::Insert(text.into()))), 0.0);
    };
    write(ServerField::Address, "superfik.acme.io");
    write(ServerField::Name, "ana");
    if first_access {
        write(ServerField::Code, "7KQM-3XRT-9HVA");
    }
    s.apply(Action::ServerLink(ServerAction::Focus(ServerField::Password)), 0.0);
    s.notice = None;
    (s, Vec::new())
}

pub fn users() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = governance();
    let code = ServerEvent::FirstAccessCode { user: UserId::new(), name: "bia".into(), code: "7KQM-3XRT-9HVA".into() };
    s.apply(Action::Server(code), 0.0);
    let joined = crate::state::Joined { address: "superfik.acme.io".into(), name: "olga".into() };
    s.apply(Action::ServerLink(crate::state::ServerAction::Joined(Ok(joined))), 0.0);
    (s, images)
}
