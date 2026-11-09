//! The form that writes a routine: its name, brief, cadence and autonomy.

use superfik_core::Connection;
use superfik_core::days::{local_day, weekday};
use superfik_core::routine::{Autonomy, Routine, RoutineDraft, Schedule, Template};
use superfik_ui::TextEdit;

pub fn template_name(template: Template) -> superfik_i18n::Text<'static> {
    match template {
        Template::ProductionUpkeep => superfik_i18n::Text::TemplateProductionUpkeep,
        Template::ReleaseNotes => superfik_i18n::Text::TemplateReleaseNotes,
        Template::RevenueDigest => superfik_i18n::Text::TemplateRevenueDigest,
        Template::CustomerSupport => superfik_i18n::Text::TemplateCustomerSupport,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoutineField {
    Name,
    Brief,
}

impl RoutineField {
    pub const ALL: [RoutineField; 2] = [RoutineField::Name, RoutineField::Brief];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cadence {
    Hourly,
    Daily,
    Weekly,
}

pub const DEFAULT_MINUTE: u16 = 9 * 60;

impl Cadence {
    pub const ALL: [Cadence; 3] = [Cadence::Hourly, Cadence::Daily, Cadence::Weekly];

    pub fn of(schedule: Schedule) -> Self {
        match schedule {
            Schedule::Every { .. } => Self::Hourly,
            Schedule::Daily { .. } => Self::Daily,
            Schedule::Weekly { .. } => Self::Weekly,
        }
    }

    pub fn schedule(self, unix: i64, utc_offset: i32) -> Schedule {
        match self {
            Self::Hourly => Schedule::Every { minutes: 60 },
            Self::Daily => Schedule::Daily { minute: DEFAULT_MINUTE, utc_offset },
            Self::Weekly => Schedule::Weekly {
                weekday: weekday(local_day(unix, utc_offset)) as u8,
                minute: DEFAULT_MINUTE,
                utc_offset,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoutineForm {
    pub editing: Option<superfik_core::routine::RoutineId>,
    pub name: TextEdit,
    pub brief: TextEdit,
    pub focus: RoutineField,
    pub schedule: Schedule,
    pub autonomy: Autonomy,
    pub model: Option<String>,
    pub connections: Vec<Connection>,
    pub answers: bool,
    pub webhook: bool,
}

impl RoutineForm {
    pub fn new(model: Option<String>, unix: i64, utc_offset: i32) -> Self {
        Self {
            editing: None,
            name: TextEdit::new(String::new()),
            brief: TextEdit::new(String::new()),
            focus: RoutineField::Name,
            schedule: Cadence::Daily.schedule(unix, utc_offset),
            autonomy: Autonomy::Draft,
            model,
            connections: Vec::new(),
            answers: false,
            webhook: false,
        }
    }

    pub fn apply(&mut self, template: Template, name: String, accounts: &[Connection], utc_offset: i32) {
        if self.editing.is_some() {
            return;
        }
        let draft = template.draft(name, utc_offset);
        self.name.set(draft.name);
        self.brief.set(draft.brief);
        self.schedule = draft.schedule;
        self.autonomy = draft.autonomy;
        self.answers = draft.answers;
        self.connections =
            accounts.iter().filter(|c| template.plugins().contains(&c.plugin.0.as_str())).cloned().collect();
    }

    pub fn toggle(&mut self, account: &Connection) {
        if let Some(at) = self.connections.iter().position(|c| c.id == account.id) {
            self.connections.remove(at);
        } else {
            self.connections.push(account.clone());
        }
    }

    pub fn revise(routine: &Routine) -> Self {
        Self {
            editing: Some(routine.id),
            name: TextEdit::new(routine.name.clone()),
            brief: TextEdit::new(routine.brief.clone()),
            focus: RoutineField::Name,
            schedule: routine.schedule,
            autonomy: routine.autonomy,
            model: routine.model.clone(),
            connections: routine.connections.clone(),
            answers: routine.answers,
            webhook: routine.webhook.is_some(),
        }
    }

    pub fn field_mut(&mut self, field: RoutineField) -> &mut TextEdit {
        match field {
            RoutineField::Name => &mut self.name,
            RoutineField::Brief => &mut self.brief,
        }
    }

    pub fn choose(&mut self, cadence: Cadence, unix: i64, utc_offset: i32) {
        if Cadence::of(self.schedule) != cadence {
            self.schedule = cadence.schedule(unix, utc_offset);
        }
    }

    pub fn is_complete(&self) -> bool {
        !self.name.text().trim().is_empty() && !self.brief.text().trim().is_empty()
    }

    pub fn draft(&self) -> RoutineDraft {
        RoutineDraft {
            name: self.name.text().trim().to_owned(),
            brief: self.brief.text().trim().to_owned(),
            schedule: self.schedule,
            autonomy: self.autonomy,
            model: self.model.clone(),
            connections: self.connections.clone(),
            answers: self.answers,
        }
    }
}
