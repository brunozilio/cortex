//! A paused agent asking the user: permission to run a tool, a choice
//! between options, and a session waiting in the background.

use superfik_core::{AgentAsk, AskId, AskKind, Choice, EngineEvent, Question};
use superfik_protocol::ServerEvent;

use super::scenarios::{SyntheticImage, reference, sessions};
use crate::state::{Action, Session};

fn ask(s: &mut Session, index: usize, id: &str, kind: AskKind) {
    let Some(run) = s.conversations[index].run.as_ref().map(|r| r.id) else { return };
    let event = EngineEvent::Asked(AgentAsk { id: AskId(id.into()), kind });
    s.apply(Action::Server(ServerEvent::Run { run, event }), 0.0);
}

fn bash(command: &str) -> AskKind {
    AskKind::Permission { tool: "Bash".into(), subject: command.into(), rememberable: true }
}

fn active(s: &Session) -> usize {
    s.conversations.iter().position(|c| c.id == s.active).unwrap_or(0)
}

pub fn permission() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    let index = active(&s);
    ask(&mut s, index, "r1", bash("cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings"));
    (s, images)
}

pub fn permission_queue() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = permission();
    let index = active(&s);
    let edit = AskKind::Permission { tool: "Write".into(), subject: "src/sky/stars.rs".into(), rememberable: false };
    ask(&mut s, index, "r2", edit);
    (s, images)
}

pub fn question_choice() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    let index = active(&s);
    let choice = |label: &str, description: &str| Choice { label: label.into(), description: description.into() };
    let questions = vec![
        Question {
            header: "Star field".into(),
            text: "How should the stars be drawn?".into(),
            options: vec![
                choice("Dithered dots", "Ordered dither, matches the minis' style"),
                choice("Soft glow", "Blurred sprites with additive blending"),
                choice("Pixel crosses", "Four-point sparkles on a 3×3 grid"),
            ],
            multi: false,
        },
        Question {
            header: "Extras".into(),
            text: "Anything else in the sky?".into(),
            options: vec![choice("Moon", ""), choice("Shooting stars", ""), choice("Clouds", "")],
            multi: true,
        },
    ];
    ask(&mut s, index, "q1", AskKind::Questions(questions));
    s.apply(Action::MoveChoice(1), 0.0);
    (s, images)
}

pub fn waiting_sessions() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = sessions();
    let Some(index) = s.conversations.iter().position(|c| c.is_running() && c.id != s.active) else {
        return (s, images);
    };
    ask(&mut s, index, "r1", bash("npm run deploy"));
    (s, images)
}

pub fn full_access() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.apply(Action::SetPermissionMode(superfik_core::PermissionMode::Bypass), 0.0);
    (s, images)
}

pub fn permission_menu() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    if let Some(c) = s.conversations.iter_mut().find(|c| c.id == s.active) {
        c.run = None;
    }
    s.popover = Some(crate::state::PopoverKind::Permission);
    (s, images)
}
