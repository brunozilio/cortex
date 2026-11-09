//! The Routines screen: two routines (one paused) and their reports, one
//! opened; and the form writing a new routine from a template, with the
//! connected accounts to pick from.

use superfik_core::routine::{Autonomy, Ending, RoutineBook, RoutineDraft, RoutineEdit, RoutineId, Schedule, Template};
use superfik_core::{Connection, ConnectionId, PluginId, RunId};
use superfik_protocol::ServerEvent;

use super::scenarios::SyntheticImage;
use crate::state::{Action, RoutineAction, Session, WallClock};

const CLOCK: WallClock = WallClock { unix_at_zero: 1_793_106_000, utc_offset_secs: -3 * 3_600 };
const HOUR: i64 = 3_600;

fn account(plugin: &str, name: &str, label: &str) -> Connection {
    Connection {
        id: ConnectionId::new(),
        plugin: PluginId(plugin.into()),
        plugin_name: name.into(),
        label: label.into(),
        icon: String::new(),
    }
}

fn accounts() -> Vec<Connection> {
    vec![
        account("sentry", "Sentry", "acme"),
        account("linear", "Linear", "acme"),
        account("github", "GitHub", ""),
        account("gmail", "Gmail", "support"),
        account("whatsapp", "WhatsApp", "support"),
    ]
}

fn book(opened: RunId, accounts: &[Connection]) -> RoutineBook {
    let at = CLOCK.unix_at_zero;
    let offset = CLOCK.utc_offset_secs;
    let mut book = RoutineBook::default();
    let upkeep = RoutineDraft {
        name: "Production upkeep".into(),
        connections: accounts[..3].to_vec(),
        ..Template::ProductionUpkeep.draft(String::new(), offset)
    };
    let changelog = RoutineDraft {
        name: "Weekly changelog".into(),
        brief: "Summarise the pull requests merged this week as release notes and a post for X.".into(),
        schedule: Schedule::Weekly { weekday: 4, minute: 17 * 60, utc_offset: offset },
        autonomy: Autonomy::Draft,
        model: None,
        connections: Vec::new(),
        answers: false,
    };
    let edits = [RoutineEdit::Create { draft: upkeep }, RoutineEdit::Create { draft: changelog }];
    book.apply(&edits, at - 30 * 24 * HOUR);
    book.apply(&[RoutineEdit::Enable { id: RoutineId(1), enabled: false }], at - HOUR);
    book.apply(&[RoutineEdit::Webhook { id: RoutineId(0), on: true }], at - HOUR);
    let firings = [
        (RoutineId(1), at - 4 * 24 * HOUR - 17 * HOUR, Ending::Completed, "Release notes drafted for 1.8.0.", true),
        (RoutineId(0), at - 25 * HOUR, Ending::Failed("claude: not logged in".into()), "", true),
        (
            RoutineId(0),
            at - HOUR,
            Ending::Completed,
            "Two new issues since yesterday.\n\n1. TypeError in checkout (regression from #412): the coupon \
             field can be null. Opened SUP-131; fixed in `applyCoupon` with a test, committed on the branch.\n\
             2. Timeout on /export for large workspaces: known, tracked in SUP-88.",
            false,
        ),
    ];
    for (routine, started, ending, text, read) in firings {
        let run = if read { RunId::new() } else { opened };
        let branch = (routine == RoutineId(0)).then(|| "routine/production-upkeep-20261027-1200".to_owned());
        book.start(routine, run, branch, started);
        book.finish(run, ending, text, 0.04, started + 180);
        if read {
            book.apply(&[RoutineEdit::Read { run }], started + 200);
        }
    }
    book.start(RoutineId(0), RunId::new(), None, at - 60);
    book
}

pub fn routines() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = super::scenario_claude::claude();
    s.notice = None;
    s.clock = CLOCK;
    s.plugins.connections = accounts();
    let opened = RunId::new();
    let path = s.root.clone().unwrap_or_default();
    let book = book(opened, &s.plugins.connections);
    s.apply(Action::Server(ServerEvent::Routines { path, book }), 0.0);
    let whatsapp = s.plugins.connections[4].id;
    let draft = superfik_core::channel::Draft {
        id: 0,
        exchange: "5511987654321".into(),
        text: "Oi Ana! Vi aqui: o erro no checkout com cupom é nosso, já está sendo corrigido. \
               Te aviso assim que sair a correção."
            .into(),
        run: RunId::new(),
        written: CLOCK.unix_at_zero * 1_000,
        attachment: Some(superfik_core::channel::Attachment::of("/repo/docs/cupom-corrigido.png".into(), 48_213)),
    };
    let linked = superfik_core::channel::LinkStatus::Linked { account: "5511999990000".into(), hook: None };
    s.apply(Action::Server(ServerEvent::Channel { connection: whatsapp, status: linked, drafts: vec![draft] }), 0.0);
    s.apply(Action::Routines(RoutineAction::Open), 0.0);
    s.apply(Action::Routines(RoutineAction::Select(opened)), 0.0);
    (s, images)
}

pub fn routines_new() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = routines();
    s.apply(Action::Routines(RoutineAction::New), 0.0);
    s.apply(Action::Routines(RoutineAction::Template(Template::ProductionUpkeep)), 0.0);
    (s, images)
}

pub fn routines_draft() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = routines();
    s.apply(Action::Routines(RoutineAction::Select(RunId::new())), 0.0);
    let whatsapp = s.plugins.connections[4].id;
    s.apply(Action::Routines(RoutineAction::EditDraft(whatsapp, 0)), 0.0);
    (s, images)
}

pub fn routines_webhook() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = routines();
    s.apply(Action::Routines(RoutineAction::Revise(RoutineId(0))), 0.0);
    s.routines.copied = Some((RoutineId(0), 0.0));
    (s, images)
}
