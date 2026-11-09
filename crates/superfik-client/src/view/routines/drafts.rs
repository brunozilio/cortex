//! Replies agents drafted on channel accounts, waiting for a member: each
//! with its account and conversation, the text, Send and Discard. Shown
//! above the routines only while there are some.

use superfik_i18n::Text;
use superfik_render::{Icon, Rect};
use superfik_ui::Painter;
use superfik_ui::components::{CardControl, CardRow, FormCard, Leading, TextField};
use superfik_ui::icons;
use superfik_ui::theme::metrics;

use super::RoutinesScreenView;
use crate::view::target::{HitMap, RoutinesTarget, Target};

const SHOWN_CHARS: usize = 140;

impl RoutinesScreenView<'_> {
    pub(super) fn paint_drafts(&self, p: &mut Painter, column: Rect, y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let drafts = self.session.channels.drafts();
        if drafts.is_empty() {
            return y;
        }
        let strings = self.session.strings();
        let (send, discard) = (strings.t(Text::DraftSend), strings.t(Text::DraftDiscard));
        let buttons: [(&str, Icon); 2] = [(send.as_str(), icons::CHECK), (discard.as_str(), icons::CLOSE)];
        let titles: Vec<String> = drafts
            .iter()
            .map(|(connection, draft)| {
                let account = self.session.plugins.connections.iter().find(|c| c.id == *connection);
                let account = account.map_or_else(String::new, superfik_core::Connection::title);
                format!("{account} · {}", draft.exchange)
            })
            .collect();
        let texts: Vec<String> = drafts
            .iter()
            .map(|(_, draft)| match draft.attachment.as_ref().and_then(|a| a.media.filename.as_deref()) {
                Some(name) if draft.text.trim().is_empty() => name.to_owned(),
                Some(name) => format!("{name} · {}", clip(&draft.text)),
                None => clip(&draft.text),
            })
            .collect();
        let glyphs: Vec<Icon> = drafts
            .iter()
            .map(|(_, draft)| match draft.attachment.as_ref().map(|a| a.media.kind) {
                Some(superfik_core::channel::MediaKind::Image) => icons::IMAGE,
                Some(_) => icons::DOCUMENT,
                None => icons::CHAT,
            })
            .collect();
        let cards: Vec<CardRow<'_>> = titles
            .iter()
            .zip(&texts)
            .zip(&glyphs)
            .map(|((title, text), glyph)| CardRow {
                leading: Some(Leading::Glyph(*glyph)),
                title,
                detail: Some(text),
                control: CardControl::Buttons(&buttons),
            })
            .collect();
        let hovered = drafts.iter().enumerate().find_map(|(row, (connection, draft))| match self.hovered {
            Some(Target::Routines(RoutinesTarget::ApproveDraft(c, d))) if c == *connection && d == draft.id => {
                Some((row, 0))
            }
            Some(Target::Routines(RoutinesTarget::DiscardDraft(c, d))) if c == *connection && d == draft.id => {
                Some((row, 1))
            }
            _ => None,
        });
        let heading = strings.t(Text::DraftsHeading);
        let card = FormCard { heading: Some(&heading), rows: &cards, hovered };
        let height = card.height();
        let parts = card.paint(p, Rect::new(column.x, y, column.w, height));
        let mut row_y = y + metrics::FORM_HEADING;
        for ((connection, draft), row) in drafts.iter().zip(&cards) {
            let rect = Rect::new(column.x, row_y, column.w, row.height());
            hits.add_screen(
                rect.intersect(&visible),
                Target::Routines(RoutinesTarget::EditDraft(*connection, draft.id)),
            );
            row_y += row.height();
        }
        for ((connection, draft), rects) in drafts.iter().zip(parts) {
            let targets = [
                RoutinesTarget::ApproveDraft(*connection, draft.id),
                RoutinesTarget::DiscardDraft(*connection, draft.id),
            ];
            for (rect, target) in rects.into_iter().zip(targets) {
                hits.add_screen(rect.intersect(&visible), Target::Routines(target));
            }
        }
        let mut y = y + height;
        if let Some(edit) = &self.session.channels.editing {
            y += metrics::FORM_FIELD_GAP;
            let field = Rect::new(column.x, y, column.w, metrics::FORM_FIELD_MULTILINE);
            TextField {
                edit: &edit.text,
                placeholder: "",
                focused: true,
                caret_visible: self.caret_visible,
                multiline: true,
                secret: false,
            }
            .paint(p, field);
            hits.add_screen(field.intersect(&visible), Target::Routines(RoutinesTarget::DraftField));
            y += field.h;
        }
        y + metrics::FORM_GROUP_GAP
    }
}

fn clip(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= SHOWN_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(SHOWN_CHARS).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_draft_is_one_line_cut_with_an_ellipsis() {
        assert_eq!(clip("Oi Ana,\n\njá estou vendo."), "Oi Ana, já estou vendo.");
        let long = "x".repeat(SHOWN_CHARS + 10);
        assert_eq!(clip(&long).chars().count(), SHOWN_CHARS + 1);
        assert!(clip(&long).ends_with('…'));
    }
}
