//! The webhook card of a saved routine's form: the switch, and once the
//! routine has an address, the button that copies a call to it.

use superfik_i18n::Text;
use superfik_render::Rect;
use superfik_ui::components::{CardControl, CardRow, FormCard, Progress};
use superfik_ui::icons;
use superfik_ui::theme::metrics;

use super::RoutinesScreenView;
use crate::state::routines::RoutineForm;
use crate::view::target::{HitMap, RoutinesTarget, Target};

impl RoutinesScreenView<'_> {
    pub(super) fn paint_webhook(
        &self,
        p: &mut superfik_ui::Painter,
        form: &RoutineForm,
        column: Rect,
        y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let Some(id) = form.editing else { return y };
        let state = &self.session.routines;
        let strings = self.session.strings();
        let title = strings.t(Text::RoutineWebhook);
        let detail = strings.t(Text::RoutineWebhookDetail);
        let call = strings.t(Text::RoutineWebhookCall);
        let copy = strings.t(Text::RoutineCopyCall);
        let copied = strings.t(Text::LinkCopied);
        let hook = state.book.get(id).and_then(|routine| routine.webhook).filter(|_| form.webhook);
        let address = hook.map(|hook| format!("POST /hooks/{hook}"));
        let progress = match state.copied {
            _ if state.copying == Some(id) => Progress::Busy { age: 0.0, time: self.now as f32 },
            Some((routine, at)) if routine == id => Progress::Done { age: (self.now - at) as f32, after_busy: true },
            _ => Progress::Idle,
        };
        let mut cards = vec![CardRow {
            leading: None,
            title: &title,
            detail: Some(&detail),
            control: CardControl::Switch { on: form.webhook, enabled: true },
        }];
        if let Some(address) = &address {
            cards.push(CardRow {
                leading: None,
                title: &call,
                detail: Some(address),
                control: CardControl::Button {
                    labels: [&copy, &copy, &copied],
                    icon: icons::BOOKMARK,
                    enabled: true,
                    progress,
                },
            });
        }
        let hovered = match self.hovered {
            Some(Target::Routines(RoutinesTarget::Webhook)) => Some((0, 0)),
            Some(Target::Routines(RoutinesTarget::CopyWebhook(_))) => Some((1, 0)),
            _ => None,
        };
        let card = FormCard { heading: None, rows: &cards, hovered };
        let height = card.height();
        let parts = card.paint(p, Rect::new(column.x, y, column.w, height));
        for (row, rects) in parts.into_iter().enumerate() {
            let target = if row == 0 { RoutinesTarget::Webhook } else { RoutinesTarget::CopyWebhook(id) };
            for rect in rects {
                hits.add_screen(rect.intersect(&visible), Target::Routines(target));
            }
        }
        y + height + metrics::FORM_FIELD_GAP
    }
}
