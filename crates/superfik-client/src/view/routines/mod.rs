//! The Routines screen (ADR 0012), over the whole window like Settings:
//! the shared header, then one column: the project's routines as a card,
//! the form that writes one, and the reports of their firings, the chosen
//! one shown whole. Painted in window coordinates; every region is a
//! screen hit so the canvas under it is inert.

mod drafts;
mod form;
pub mod rows;
mod webhook;

use superfik_core::routine::{Report, Routine};
use superfik_i18n::Text;
use superfik_render::{Icon, Point, Quad, Rect};
use superfik_ui::components::{CardControl, CardRow, Chip, FormCard, ScreenHeader};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, Plane, icons};

use super::target::{HitMap, RoutinesTarget, Target};
use crate::state::Session;

const SHOWN_REPORTS: usize = 30;

pub struct RoutinesScreenView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub now: f64,
    pub caret_visible: bool,
}

impl RoutinesScreenView<'_> {
    fn is(&self, target: RoutinesTarget) -> bool {
        self.hovered == Some(Target::Routines(target))
    }

    pub fn paint(&self, p: &mut Painter, hits: &mut HitMap) {
        if !self.session.routines.open {
            return;
        }
        let strings = self.session.strings();
        let window = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        p.on_plane(Plane::Canvas, |p| {
            p.quad(Quad::new(window, 0.0, p.palette().canvas));
            hits.add_screen(window, Target::Routines(RoutinesTarget::Surface));
            let back = strings.t(Text::ReviewBack);
            let header = ScreenHeader {
                top_inset: self.session.platform.titlebar_height(),
                back: &back,
                back_hovered: self.is(RoutinesTarget::Back),
            }
            .paint(p, window);
            hits.add_screen(header.back, Target::Routines(RoutinesTarget::Back));
            ScreenHeader::title(p, &header, &strings.t(Text::RoutinesTitle), header.actions_right);
            let body = ScreenHeader::body(window, self.session.platform.titlebar_height());
            let room = (body.w - metrics::SCREEN_CONTENT_INSET * 2.0).max(0.0);
            let width = room.min(metrics::SETTINGS_CONTENT_WIDTH);
            let column = Rect::new((body.x + (body.w - width) * 0.5).round(), body.y, width, body.h);
            let visible = Rect::new(window.x, body.y, window.w, window.bottom() - body.y);
            p.clipped(visible, |p| {
                let scroll = self.session.routines.scroll;
                let top = column.y + metrics::SETTINGS_TOP;
                let bottom = self.paint_column(p, column, top - scroll, visible, hits);
                let content = bottom + scroll - top;
                hits.routines_scroll = Some((visible, (content - (visible.bottom() - top)).max(0.0)));
            });
        });
    }

    fn paint_column(&self, p: &mut Painter, column: Rect, mut y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let strings = self.session.strings();
        let c = p.palette();
        let title = p.label(&strings.t(Text::RoutinesTitle), typography::STORE_DISPLAY, column.x, y + 16.0, c.text);
        y += title.height.max(32.0);
        let caption = p.layout(&strings.t(Text::RoutinesCaption), typography::STORE_BODY, Some(column.w));
        p.text(&caption, Point::new(column.x, y), c.text_secondary);
        y += caption.height + metrics::SETTINGS_INTRO_GAP;
        y = self.paint_drafts(p, column, y, visible, hits);
        y = self.paint_routines(p, column, y, visible, hits);
        let form = self.session.routines.form.as_ref();
        if form.is_none() {
            let rect = self.chip(p, Point::new(column.x, y), icons::PLUS, Text::RoutineNew, RoutinesTarget::New);
            hits.add_screen(rect.intersect(&visible), Target::Routines(RoutinesTarget::New));
            y += metrics::ROW_HEIGHT;
        }
        y += metrics::FORM_GROUP_GAP;
        if let Some(form) = form {
            y = self.paint_form(p, form, column, y, visible, hits) + metrics::FORM_GROUP_GAP;
        }
        self.paint_reports(p, column, y, visible, hits)
    }

    fn paint_routines(&self, p: &mut Painter, column: Rect, y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let strings = self.session.strings();
        let routines = &self.session.routines.book.routines;
        let none = strings.t(Text::RoutinesNone);
        let (run, pause, resume, edit) = (
            strings.t(Text::RoutineRunNow),
            strings.t(Text::RoutinePause),
            strings.t(Text::RoutineResume),
            strings.t(Text::RoutineEdit),
        );
        let details: Vec<String> = routines.iter().map(|r| rows::routine_detail(&strings, r)).collect();
        let buttons: Vec<[(&str, Icon); 3]> = routines
            .iter()
            .map(|r| {
                let toggle = if r.enabled { (pause.as_str(), icons::STOP) } else { (resume.as_str(), icons::PLAY) };
                [(run.as_str(), icons::BOLT), toggle, (edit.as_str(), icons::PENCIL)]
            })
            .collect();
        let mut cards: Vec<CardRow<'_>> = routines
            .iter()
            .zip(&details)
            .zip(&buttons)
            .map(|((routine, detail), buttons)| CardRow {
                leading: Some(superfik_ui::components::Leading::Glyph(icons::LOOP)),
                title: &routine.name,
                detail: Some(detail),
                control: CardControl::Buttons(buttons),
            })
            .collect();
        if cards.is_empty() {
            cards.push(CardRow { leading: None, title: &none, detail: None, control: CardControl::None });
        }
        let hovered = routines.iter().enumerate().find_map(|(row, r)| match self.hovered {
            Some(Target::Routines(RoutinesTarget::RunNow(id))) if id == r.id => Some((row, 0)),
            Some(Target::Routines(RoutinesTarget::Toggle(id))) if id == r.id => Some((row, 1)),
            Some(Target::Routines(RoutinesTarget::Revise(id))) if id == r.id => Some((row, 2)),
            _ => None,
        });
        let heading = strings.t(Text::RoutinesHeading);
        let card = FormCard { heading: Some(&heading), rows: &cards, hovered };
        let height = card.height();
        let parts = card.paint(p, Rect::new(column.x, y, column.w, height));
        for (routine, rects) in routines.iter().zip(parts) {
            let targets = [
                RoutinesTarget::RunNow(routine.id),
                RoutinesTarget::Toggle(routine.id),
                RoutinesTarget::Revise(routine.id),
            ];
            for (rect, target) in rects.into_iter().zip(targets) {
                hits.add_screen(rect.intersect(&visible), Target::Routines(target));
            }
        }
        y + height + metrics::FORM_FIELD_GAP
    }

    fn paint_reports(&self, p: &mut Painter, column: Rect, mut y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let strings = self.session.strings();
        let state = &self.session.routines;
        let unix = self.session.clock.wall_now(self.now);
        let offset = self.session.clock.utc_offset_secs;
        let reports: Vec<&Report> = state.book.reports.iter().take(SHOWN_REPORTS).collect();
        let name = |report: &Report| state.book.get(report.routine).map_or("", |r: &Routine| r.name.as_str());
        let details: Vec<String> = reports.iter().map(|r| rows::report_detail(&strings, r, unix, offset)).collect();
        let none = strings.t(Text::ReportsNone);
        let mut cards: Vec<CardRow<'_>> = reports
            .iter()
            .zip(&details)
            .map(|(report, detail)| CardRow {
                leading: Some(superfik_ui::components::Leading::Glyph(if report.read || report.is_running() {
                    icons::DOCUMENT
                } else {
                    icons::SPARKLE
                })),
                title: name(report),
                detail: Some(detail),
                control: CardControl::None,
            })
            .collect();
        if cards.is_empty() {
            cards.push(CardRow { leading: None, title: &none, detail: None, control: CardControl::None });
        }
        let heading = strings.t(Text::ReportsHeading);
        let card = FormCard { heading: Some(&heading), rows: &cards, hovered: None };
        let height = card.height();
        card.paint(p, Rect::new(column.x, y, column.w, height));
        let mut row_y = y + metrics::FORM_HEADING;
        for (report, row) in reports.iter().zip(&cards) {
            let rect = Rect::new(column.x, row_y, column.w, row.height());
            if state.selected == Some(report.run) || self.is(RoutinesTarget::Report(report.run)) {
                p.quad(Quad::new(rect.inset(2.0), metrics::ROW_RADIUS, p.palette().timeline_hover));
            }
            hits.add_screen(rect.intersect(&visible), Target::Routines(RoutinesTarget::Report(report.run)));
            row_y += row.height();
        }
        y += height + metrics::FORM_FIELD_GAP;
        if let Some(report) = state.selected.and_then(|run| state.book.report(run)) {
            y = self.paint_report(p, report, column, y, visible, hits);
        }
        y
    }

    fn paint_report(
        &self,
        p: &mut Painter,
        report: &Report,
        column: Rect,
        mut y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        if let Some(branch) = &report.branch {
            let line = self.session.strings().t(Text::ReportBranch(branch));
            p.label(&line, typography::CAPTION, column.x, y + metrics::FORM_FIELD_LABEL * 0.4, p.palette().text_subtle);
            y += metrics::FORM_FIELD_LABEL;
        }
        let text = match (&report.ending, report.text.trim()) {
            (Some(superfik_core::routine::Ending::Failed(why)), "") => why.clone(),
            (_, text) => text.to_owned(),
        };
        if !text.is_empty() {
            let layout = p.layout(&text, typography::STORE_BODY, Some(column.w));
            p.text(&layout, Point::new(column.x, y), p.palette().text);
            y += layout.height + metrics::FORM_FIELD_GAP;
        }
        if !report.is_running() {
            let target = RoutinesTarget::Continue(report.run);
            let rect = self.chip(p, Point::new(column.x, y), icons::CHAT, Text::ReportContinue, target);
            hits.add_screen(rect.intersect(&visible), Target::Routines(target));
            y += metrics::ROW_HEIGHT;
        }
        y + metrics::FORM_GROUP_GAP
    }

    fn chip(&self, p: &mut Painter, at: Point, icon: Icon, text: Text<'_>, target: RoutinesTarget) -> Rect {
        let label = self.session.strings().t(text);
        Chip { icon, label: &label, active: false, hovered: self.is(target), tint: None }.paint(p, at)
    }
}
