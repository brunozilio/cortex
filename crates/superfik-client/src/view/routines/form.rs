//! The form that writes a routine: name, brief, cadence and autonomy, then
//! Save, Cancel and (for a routine that exists) Delete.

use superfik_core::routine::{Autonomy, Template};
use superfik_i18n::Text;
use superfik_render::{Icon, Point, Rect};
use superfik_ui::Painter;
use superfik_ui::components::{CardControl, CardRow, Chip, FormCard, TextField};
use superfik_ui::icons;
use superfik_ui::theme::{metrics, typography};

use super::{RoutinesScreenView, rows};
use crate::state::routines::{Cadence, RoutineField, RoutineForm, template_name};
use crate::view::target::{HitMap, RoutinesTarget, Target};

struct Pill {
    icon: Icon,
    label: String,
    active: bool,
    target: RoutinesTarget,
}

fn template_icon(template: Template) -> Icon {
    match template {
        Template::ProductionUpkeep => icons::SHIELD,
        Template::ReleaseNotes => icons::DOCUMENT,
        Template::RevenueDigest => icons::GAUGE,
        Template::CustomerSupport => icons::CHAT,
    }
}

impl RoutinesScreenView<'_> {
    pub(super) fn paint_form(
        &self,
        p: &mut Painter,
        form: &RoutineForm,
        column: Rect,
        mut y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let strings = self.session.strings();
        if form.editing.is_none() {
            y = self.caption(p, column, y, Text::RoutineStartFrom);
            let templates: Vec<Pill> = Template::ALL
                .iter()
                .map(|t| Pill {
                    icon: template_icon(*t),
                    label: strings.t(template_name(*t)),
                    active: false,
                    target: RoutinesTarget::Template(*t),
                })
                .collect();
            y = self.pills(p, &templates, column, y, visible, hits) + metrics::FORM_FIELD_GAP;
        }
        for (field, label, placeholder, height) in [
            (RoutineField::Name, Text::RoutineName, Text::RoutineNamePlaceholder, metrics::ROW_HEIGHT),
            (RoutineField::Brief, Text::RoutineBrief, Text::RoutineBriefPlaceholder, metrics::FORM_FIELD_MULTILINE),
        ] {
            let c = p.palette();
            p.label(
                &strings.t(label),
                typography::CAPTION,
                column.x,
                y + metrics::FORM_FIELD_LABEL * 0.4,
                c.text_subtle,
            );
            y += metrics::FORM_FIELD_LABEL;
            let rect = Rect::new(column.x, y, column.w, height);
            let edit = match field {
                RoutineField::Name => &form.name,
                RoutineField::Brief => &form.brief,
            };
            TextField {
                edit,
                placeholder: &strings.t(placeholder),
                focused: form.focus == field,
                caret_visible: self.caret_visible,
                multiline: field == RoutineField::Brief,
                secret: false,
            }
            .paint(p, rect);
            hits.add_screen(rect.intersect(&visible), Target::Routines(RoutinesTarget::Field(field)));
            y += height + metrics::FORM_FIELD_GAP;
        }
        y = self.paint_choices(p, form, column, y, visible, hits);
        y = self.paint_webhook(p, form, column, y, visible, hits);
        y = self.paint_accounts(p, form, column, y, visible, hits) + metrics::FORM_FIELD_GAP;
        let save = self.chip(p, Point::new(column.x, y), icons::CHECK, Text::RoutineSave, RoutinesTarget::Save);
        if form.is_complete() {
            hits.add_screen(save.intersect(&visible), Target::Routines(RoutinesTarget::Save));
        }
        let at = Point::new(save.right() + metrics::ROW_GAP, y);
        let cancel = self.chip(p, at, icons::CLOSE, Text::RoutineCancel, RoutinesTarget::Cancel);
        hits.add_screen(cancel.intersect(&visible), Target::Routines(RoutinesTarget::Cancel));
        if let Some(id) = form.editing {
            let at = Point::new(cancel.right() + metrics::ROW_GAP, y);
            let delete = self.chip(p, at, icons::DELETE, Text::RoutineDelete, RoutinesTarget::Delete(id));
            hits.add_screen(delete.intersect(&visible), Target::Routines(RoutinesTarget::Delete(id)));
        }
        y + metrics::ROW_HEIGHT
    }

    fn paint_accounts(
        &self,
        p: &mut Painter,
        form: &RoutineForm,
        column: Rect,
        y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let strings = self.session.strings();
        let y = self.caption(p, column, y, Text::RoutineAccounts);
        let accounts = &self.session.plugins.connections;
        if accounts.is_empty() {
            let hint = p.layout(&strings.t(Text::RoutineAccountsNone), typography::CAPTION, Some(column.w));
            p.text(&hint, Point::new(column.x, y), p.palette().text_secondary);
            return y + hint.height;
        }
        let pills: Vec<Pill> = accounts
            .iter()
            .map(|account| Pill {
                icon: icons::PLUG,
                label: account.title(),
                active: form.connections.iter().any(|c| c.id == account.id),
                target: RoutinesTarget::Account(account.id),
            })
            .collect();
        self.pills(p, &pills, column, y, visible, hits)
    }

    fn caption(&self, p: &mut Painter, column: Rect, y: f32, text: Text<'_>) -> f32 {
        let label = self.session.strings().t(text);
        p.label(&label, typography::CAPTION, column.x, y + metrics::FORM_FIELD_LABEL * 0.4, p.palette().text_subtle);
        y + metrics::FORM_FIELD_LABEL
    }

    fn pills(&self, p: &mut Painter, pills: &[Pill], column: Rect, y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let (mut x, mut y) = (column.x, y);
        for pill in pills {
            let chip = Chip {
                icon: pill.icon,
                label: &pill.label,
                active: pill.active,
                hovered: self.is(pill.target),
                tint: None,
            };
            let width = chip.width(p);
            if x > column.x && x + width > column.right() {
                x = column.x;
                y += metrics::CHIP_HEIGHT + metrics::ROW_GAP;
            }
            let rect = chip.paint(p, Point::new(x, y));
            hits.add_screen(rect.intersect(&visible), Target::Routines(pill.target));
            x = rect.right() + metrics::ROW_GAP;
        }
        y + metrics::CHIP_HEIGHT
    }

    fn paint_choices(
        &self,
        p: &mut Painter,
        form: &RoutineForm,
        column: Rect,
        y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let strings = self.session.strings();
        let cadences: Vec<String> = Cadence::ALL.iter().map(|c| strings.t(rows::cadence(*c))).collect();
        let autonomies: Vec<String> = Autonomy::ALL.iter().map(|a| strings.t(rows::autonomy(*a))).collect();
        let cadence_labels: Vec<&str> = cadences.iter().map(String::as_str).collect();
        let autonomy_labels: Vec<&str> = autonomies.iter().map(String::as_str).collect();
        let when = strings.t(Text::RoutineWhen);
        let schedule = rows::schedule(&strings, form.schedule);
        let latitude = strings.t(Text::RoutineAutonomy);
        let answers = strings.t(Text::RoutineAnswers);
        let answers_detail = strings.t(Text::RoutineAnswersDetail);
        let selected = |i: Option<usize>| i.unwrap_or(0);
        let cards = [
            CardRow {
                leading: None,
                title: &when,
                detail: Some(&schedule),
                control: CardControl::Choice {
                    labels: &cadence_labels,
                    selected: selected(Cadence::ALL.iter().position(|c| *c == Cadence::of(form.schedule))),
                    enabled: true,
                },
            },
            CardRow {
                leading: None,
                title: &latitude,
                detail: None,
                control: CardControl::Choice {
                    labels: &autonomy_labels,
                    selected: selected(Autonomy::ALL.iter().position(|a| *a == form.autonomy)),
                    enabled: true,
                },
            },
            CardRow {
                leading: None,
                title: &answers,
                detail: Some(&answers_detail),
                control: CardControl::Switch { on: form.answers, enabled: true },
            },
        ];
        let hovered = match self.hovered {
            Some(Target::Routines(RoutinesTarget::Cadence(c))) => {
                Cadence::ALL.iter().position(|x| *x == c).map(|i| (0, i))
            }
            Some(Target::Routines(RoutinesTarget::Autonomy(a))) => {
                Autonomy::ALL.iter().position(|x| *x == a).map(|i| (1, i))
            }
            Some(Target::Routines(RoutinesTarget::Answers)) => Some((2, 0)),
            _ => None,
        };
        let card = FormCard { heading: None, rows: &cards, hovered };
        let height = card.height();
        let parts = card.paint(p, Rect::new(column.x, y, column.w, height));
        for (row, rects) in parts.into_iter().enumerate() {
            for (i, rect) in rects.into_iter().enumerate() {
                let target = match row {
                    0 => Cadence::ALL.get(i).map(|c| RoutinesTarget::Cadence(*c)),
                    1 => Autonomy::ALL.get(i).map(|a| RoutinesTarget::Autonomy(*a)),
                    _ => Some(RoutinesTarget::Answers),
                };
                if let Some(target) = target {
                    hits.add_screen(rect.intersect(&visible), Target::Routines(target));
                }
            }
        }
        y + height + metrics::FORM_FIELD_GAP
    }
}
