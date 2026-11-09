//! The rules of a `RoutineBook`: edits, what is due, firings and endings.

use super::{
    Ending, MAX_BRIEF_CHARS, MAX_REPORT_CHARS, MAX_REPORTS, Outcome, Report, Routine, RoutineBook, RoutineDraft,
    RoutineEdit, RoutineId,
};
use crate::RunId;

impl RoutineBook {
    pub fn get(&self, id: RoutineId) -> Option<&Routine> {
        self.routines.iter().find(|r| r.id == id)
    }

    pub fn report(&self, run: RunId) -> Option<&Report> {
        self.reports.iter().find(|r| r.run == run)
    }

    pub fn unread(&self) -> usize {
        self.reports.iter().filter(|r| !r.read && !r.is_running()).count()
    }

    pub fn apply(&mut self, edits: &[RoutineEdit], now: i64) -> Outcome {
        let mut outcome = Outcome::default();
        for edit in edits {
            match edit {
                RoutineEdit::Create { draft } => {
                    let Some(draft) = clean(draft) else { continue };
                    let id = RoutineId(self.next.max(self.after_highest()));
                    self.next = id.0 + 1;
                    self.routines.push(Routine {
                        id,
                        next_run: Some(draft.schedule.next_after(now)),
                        name: draft.name,
                        brief: draft.brief,
                        schedule: draft.schedule,
                        autonomy: draft.autonomy,
                        model: draft.model,
                        connections: draft.connections,
                        answers: draft.answers,
                        webhook: None,
                        enabled: true,
                        last_run: None,
                    });
                    outcome.created.push(id);
                }
                RoutineEdit::Revise { id, draft } => {
                    let (Some(routine), Some(draft)) = (self.routines.iter_mut().find(|r| r.id == *id), clean(draft))
                    else {
                        outcome.refused.push(*id);
                        continue;
                    };
                    if routine.schedule != draft.schedule && routine.enabled {
                        routine.next_run = Some(draft.schedule.next_after(now));
                    }
                    routine.name = draft.name;
                    routine.brief = draft.brief;
                    routine.schedule = draft.schedule;
                    routine.autonomy = draft.autonomy;
                    routine.model = draft.model;
                    routine.connections = draft.connections;
                    routine.answers = draft.answers;
                    outcome.changed.push(*id);
                }
                RoutineEdit::Enable { id, enabled } => match self.routines.iter_mut().find(|r| r.id == *id) {
                    Some(routine) => {
                        if routine.enabled != *enabled {
                            routine.enabled = *enabled;
                            routine.next_run = enabled.then(|| routine.schedule.next_after(now));
                        }
                        outcome.changed.push(*id);
                    }
                    None => outcome.refused.push(*id),
                },
                RoutineEdit::RunNow { id } => match self.routines.iter_mut().find(|r| r.id == *id) {
                    Some(routine) => {
                        routine.next_run = Some(now);
                        outcome.changed.push(*id);
                    }
                    None => outcome.refused.push(*id),
                },
                RoutineEdit::Webhook { id, on } => match self.routines.iter_mut().find(|r| r.id == *id) {
                    Some(routine) => {
                        if *on != routine.webhook.is_some() {
                            routine.webhook = on.then(super::WebhookId::new);
                        }
                        outcome.changed.push(*id);
                    }
                    None => outcome.refused.push(*id),
                },
                RoutineEdit::Delete { id } => {
                    let before = self.routines.len();
                    self.routines.retain(|r| r.id != *id);
                    if self.routines.len() == before {
                        outcome.refused.push(*id);
                    } else {
                        self.reports.retain(|r| r.routine != *id || r.is_running());
                        outcome.changed.push(*id);
                    }
                }
                RoutineEdit::Read { run } => {
                    if let Some(report) = self.reports.iter_mut().find(|r| r.run == *run) {
                        report.read = true;
                        outcome.changed.push(report.routine);
                    }
                }
            }
        }
        outcome
    }

    pub fn due(&self, now: i64) -> Vec<RoutineId> {
        self.routines
            .iter()
            .filter(|r| r.next_run.is_some_and(|at| at <= now))
            .filter(|r| !self.reports.iter().any(|report| report.routine == r.id && report.is_running()))
            .map(|r| r.id)
            .collect()
    }

    pub fn start(&mut self, id: RoutineId, run: RunId, branch: Option<String>, now: i64) -> bool {
        let Some(routine) = self.routines.iter_mut().find(|r| r.id == id) else { return false };
        routine.last_run = Some(now);
        routine.next_run = routine.enabled.then(|| routine.schedule.next_after(now));
        self.report_start(id, run, branch, None, now);
        true
    }

    pub fn listening(&self, connection: crate::ConnectionId, exchange: &str) -> Vec<RoutineId> {
        self.routines
            .iter()
            .filter(|r| r.listens_to(connection))
            .filter(|r| {
                !self.reports.iter().any(|report| {
                    report.routine == r.id && report.is_running() && report.exchange.as_deref() == Some(exchange)
                })
            })
            .map(|r| r.id)
            .collect()
    }

    pub fn hooked(&self, hook: super::WebhookId) -> Option<RoutineId> {
        self.routines.iter().find(|r| r.enabled && r.webhook == Some(hook)).map(|r| r.id)
    }

    pub fn start_hooked(&mut self, id: RoutineId, run: RunId, branch: Option<String>, now: i64) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.report_start(id, run, branch, None, now);
        true
    }

    pub fn start_answer(
        &mut self,
        id: RoutineId,
        run: RunId,
        exchange: &str,
        branch: Option<String>,
        now: i64,
    ) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.report_start(id, run, branch, Some(exchange.to_owned()), now);
        true
    }

    fn report_start(&mut self, id: RoutineId, run: RunId, branch: Option<String>, exchange: Option<String>, now: i64) {
        self.reports.insert(
            0,
            Report {
                routine: id,
                run,
                started: now,
                finished: None,
                ending: None,
                text: String::new(),
                cost_usd: 0.0,
                read: false,
                branch,
                exchange,
            },
        );
        self.reports.truncate(MAX_REPORTS);
    }

    pub fn finish(&mut self, run: RunId, ending: Ending, text: &str, cost_usd: f64, now: i64) -> bool {
        let Some(report) = self.reports.iter_mut().find(|r| r.run == run && r.is_running()) else { return false };
        report.ending = Some(ending);
        report.finished = Some(now);
        report.text = text.chars().take(MAX_REPORT_CHARS).collect();
        report.cost_usd = cost_usd;
        true
    }

    pub fn settle(&mut self, now: i64) -> bool {
        let mut changed = false;
        for report in self.reports.iter_mut().filter(|r| r.is_running()) {
            report.ending = Some(Ending::Cancelled);
            report.finished = Some(now);
            changed = true;
        }
        changed
    }

    fn after_highest(&self) -> u32 {
        self.routines.iter().map(|r| r.id.0 + 1).max().unwrap_or(0)
    }
}

fn clean(draft: &RoutineDraft) -> Option<RoutineDraft> {
    let name = draft.name.trim().to_owned();
    let brief: String = draft.brief.trim().chars().take(MAX_BRIEF_CHARS).collect();
    if name.is_empty() || brief.is_empty() {
        return None;
    }
    let model = draft.model.as_ref().map(|m| m.trim().to_owned()).filter(|m| !m.is_empty());
    Some(RoutineDraft { name, brief, model, ..draft.clone() })
}
