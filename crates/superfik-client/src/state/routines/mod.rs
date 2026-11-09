//! The Routines screen (ADR 0012): the project's routines, the form that
//! writes them and the reports of their firings, newest first. The backend
//! keeps and fires them; this keeps what it last said and what is open.

mod form;
pub mod webhook;

use std::path::PathBuf;

pub use form::{Cadence, RoutineField, RoutineForm, template_name};
use superfik_core::routine::{Autonomy, RoutineBook, RoutineEdit, RoutineId, Template};
use superfik_core::{ConnectionId, RunId};
use superfik_protocol::ClientCommand;

use super::{EditKey, Effect, Session};

#[derive(Debug, Clone, Default)]
pub struct RoutinesState {
    pub open: bool,
    pub book: RoutineBook,
    pub loaded: bool,
    pub selected: Option<RunId>,
    pub form: Option<RoutineForm>,
    pub scroll: f32,
    pub copying: Option<RoutineId>,
    pub copied: Option<(RoutineId, f64)>,
}

#[derive(Clone)]
pub enum RoutineAction {
    Open,
    Close,
    Back,
    Select(RunId),
    New,
    Revise(RoutineId),
    Toggle(RoutineId),
    RunNow(RoutineId),
    Delete(RoutineId),
    Focus(RoutineField),
    Edit(EditKey),
    NextField { back: bool },
    Cadence(Cadence),
    Autonomy(Autonomy),
    Template(Template),
    Account(ConnectionId),
    ToggleAnswers,
    ToggleWebhook,
    CopyWebhook(RoutineId),
    EditDraft(ConnectionId, u32),
    ApproveDraft(ConnectionId, u32),
    DiscardDraft(ConnectionId, u32),
    Submit,
    Save,
    Continue(RunId),
    Scroll { delta: f32, limit: f32 },
}

impl std::fmt::Debug for RoutineAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RoutineAction(…)")
    }
}

impl Session {
    pub(super) fn load_routines(path: PathBuf) -> Effect {
        Effect::Command(Box::new(ClientCommand::LoadRoutines { path }))
    }

    pub fn unread_reports(&self) -> usize {
        self.routines.book.unread()
    }

    pub(super) fn routines_arrived(&mut self, path: PathBuf, book: RoutineBook) {
        if self.root.as_ref() != Some(&path) {
            return;
        }
        let state = &mut self.routines;
        if state.selected.is_some_and(|run| book.report(run).is_none()) {
            state.selected = None;
        }
        state.book = book;
        state.loaded = true;
    }

    pub(super) fn routine(&mut self, action: RoutineAction, now: f64) -> Vec<Effect> {
        let unix = self.clock.wall_now(now);
        let offset = self.clock.utc_offset_secs;
        let model = self.models.get(self.conversation().model).map(|m| m.id.clone());
        let strings = self.strings();
        if let Some(edit) = &mut self.channels.editing {
            match action {
                RoutineAction::Edit(key) => {
                    super::composer::apply_key(&mut edit.text, key);
                    return Vec::new();
                }
                RoutineAction::Submit => {
                    let (connection, draft) = (edit.connection, edit.draft);
                    return self.approve_draft(connection, draft);
                }
                RoutineAction::Back => {
                    self.channels.editing = None;
                    return Vec::new();
                }
                _ => {}
            }
        }
        if let RoutineAction::EditDraft(connection, draft) = action {
            self.edit_draft(connection, draft);
            return Vec::new();
        }
        let state = &mut self.routines;
        match action {
            RoutineAction::Open => {
                state.open = true;
                self.popover = None;
                let mut effects: Vec<Effect> = self.root.clone().map(Session::load_routines).into_iter().collect();
                effects.push(Effect::Command(Box::new(ClientCommand::ListPlugins)));
                effects.push(Session::list_channels());
                return effects;
            }
            RoutineAction::EditDraft(..) => {}
            RoutineAction::ApproveDraft(connection, draft) => return self.approve_draft(connection, draft),
            RoutineAction::DiscardDraft(connection, draft) => return self.discard_draft(connection, draft),
            RoutineAction::Template(template) => {
                if let Some(form) = &mut state.form {
                    let name = strings.t(form::template_name(template));
                    form.apply(template, name, &self.plugins.connections, offset);
                }
            }
            RoutineAction::ToggleAnswers => {
                if let Some(form) = &mut state.form {
                    form.answers = !form.answers;
                }
            }
            RoutineAction::ToggleWebhook => {
                if let Some(form) = state.form.as_mut().filter(|form| form.editing.is_some()) {
                    form.webhook = !form.webhook;
                }
            }
            RoutineAction::CopyWebhook(id) => return self.copy_webhook(id),
            RoutineAction::Account(id) => {
                if let (Some(form), Some(account)) =
                    (&mut state.form, self.plugins.connections.iter().find(|c| c.id == id))
                {
                    form.toggle(account);
                }
            }
            RoutineAction::Close => self.close_routines(),
            RoutineAction::Back if state.form.is_some() => state.form = None,
            RoutineAction::Back if state.selected.is_some() => state.selected = None,
            RoutineAction::Back => self.close_routines(),
            RoutineAction::Select(run) => {
                state.form = None;
                state.selected = if state.selected == Some(run) { None } else { Some(run) };
                if state.book.report(run).is_some_and(|r| !r.read && !r.is_running()) {
                    return self.edit_routines(vec![RoutineEdit::Read { run }]);
                }
            }
            RoutineAction::New => {
                state.selected = None;
                state.form = Some(RoutineForm::new(model, unix, offset));
                self.channels.editing = None;
            }
            RoutineAction::Revise(id) => {
                state.selected = None;
                state.form = state.book.get(id).map(RoutineForm::revise);
            }
            RoutineAction::Toggle(id) => {
                let Some(routine) = state.book.get(id) else { return Vec::new() };
                let enabled = !routine.enabled;
                return self.edit_routines(vec![RoutineEdit::Enable { id, enabled }]);
            }
            RoutineAction::RunNow(id) => return self.edit_routines(vec![RoutineEdit::RunNow { id }]),
            RoutineAction::Delete(id) => {
                if state.form.as_ref().is_some_and(|form| form.editing == Some(id)) {
                    state.form = None;
                }
                return self.edit_routines(vec![RoutineEdit::Delete { id }]);
            }
            RoutineAction::Focus(field) => {
                if let Some(form) = &mut state.form {
                    form.focus = field;
                }
            }
            RoutineAction::Edit(key) => {
                if let Some(form) = &mut state.form {
                    super::composer::apply_key(form.field_mut(form.focus), key);
                }
            }
            RoutineAction::NextField { back } => {
                if let Some(form) = &mut state.form {
                    let fields = RoutineField::ALL;
                    let at = fields.iter().position(|f| *f == form.focus).unwrap_or(0);
                    let step = if back { fields.len() - 1 } else { 1 };
                    form.focus = fields[(at + step) % fields.len()];
                }
            }
            RoutineAction::Cadence(cadence) => {
                if let Some(form) = &mut state.form {
                    form.choose(cadence, unix, offset);
                }
            }
            RoutineAction::Autonomy(autonomy) => {
                if let Some(form) = &mut state.form {
                    form.autonomy = autonomy;
                }
            }
            RoutineAction::Submit => match &mut state.form {
                Some(form) if form.focus == RoutineField::Name => form.focus = RoutineField::Brief,
                Some(_) => return self.routine(RoutineAction::Save, now),
                None => {}
            },
            RoutineAction::Save => {
                let Some(form) = state.form.as_ref().filter(|form| form.is_complete()) else { return Vec::new() };
                let draft = form.draft();
                let mut edits = vec![match form.editing {
                    Some(id) => RoutineEdit::Revise { id, draft },
                    None => RoutineEdit::Create { draft },
                }];
                if let Some(id) = form.editing {
                    let had = state.book.get(id).is_some_and(|r| r.webhook.is_some());
                    if form.webhook != had {
                        edits.push(RoutineEdit::Webhook { id, on: form.webhook });
                    }
                }
                state.form = None;
                return self.edit_routines(edits);
            }
            RoutineAction::Continue(run) => return self.continue_report(run),
            RoutineAction::Scroll { delta, limit } => {
                state.scroll = (state.scroll + delta).clamp(0.0, limit.max(0.0));
            }
        }
        Vec::new()
    }

    fn close_routines(&mut self) {
        let state = &mut self.routines;
        state.open = false;
        state.form = None;
        state.selected = None;
        state.scroll = 0.0;
        self.channels.editing = None;
    }

    fn edit_routines(&self, edits: Vec<RoutineEdit>) -> Vec<Effect> {
        let Some(path) = self.root.clone() else { return Vec::new() };
        vec![Effect::Command(Box::new(ClientCommand::EditRoutines { path, edits }))]
    }

    fn continue_report(&mut self, run: RunId) -> Vec<Effect> {
        let book = &self.routines.book;
        let Some(report) = book.report(run) else { return Vec::new() };
        let name = book.get(report.routine).map_or("", |r| r.name.as_str());
        let branch =
            report.branch.as_ref().map_or(String::new(), |b| format!("Its changes are on the branch `{b}`.\n\n"));
        let text = format!("The routine \"{name}\" reported:\n\n{}\n\n{branch}", report.text.trim());
        self.close_routines();
        let effects = self.new_conversation();
        self.conversation_mut().composer.set(text);
        effects
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_webhook;
