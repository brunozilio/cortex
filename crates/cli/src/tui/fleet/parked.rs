use std::mem::swap;

use app::Session;

use super::super::app::App;
use super::super::ask::Pending;
use super::super::input::Input;
use super::super::transcript::Transcript;
use super::super::turn::Turn;

#[derive(Default)]
pub struct Parked {
    pub session: Option<Session>,
    pub transcript: Transcript,
    pub turn: Turn,
    pub ask: Option<Pending>,
    pub title: Option<String>,
    pub ghost: Option<String>,
    pub input: Input,
    pub scroll: u16,
    pub attachments: super::super::attach::Attachments,
    pub pastes: super::super::pastes::Pastes,
    pub shell_context: Vec<String>,
    pub finished: bool,
}

impl Parked {
    pub fn trade(&mut self, app: &mut App) {
        swap(&mut self.session, &mut app.session);
        swap(&mut self.transcript, &mut app.transcript);
        swap(&mut self.turn, &mut app.turn);
        swap(&mut self.ask, &mut app.ask);
        swap(&mut self.title, &mut app.title);
        swap(&mut self.ghost, &mut app.ghost);
        swap(&mut self.input, &mut app.input);
        swap(&mut self.scroll, &mut app.scroll);
        swap(&mut self.attachments, &mut app.attachments);
        swap(&mut self.pastes, &mut app.pastes);
        swap(&mut self.shell_context, &mut app.shell_context);
    }

    pub fn touched(&self) -> bool {
        self.turn.busy() || self.session.as_ref().and_then(Session::id).is_some() || !self.transcript.entries.is_empty()
    }

    pub fn runs(&self, run: domain::RunId) -> bool {
        self.turn.run.as_ref().is_some_and(|control| control.id() == run)
    }

    pub fn is(&self, id: &app::SessionId) -> bool {
        self.session.as_ref().and_then(Session::id).as_ref() == Some(id)
    }
}
