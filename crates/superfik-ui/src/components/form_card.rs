//! A group of settings: an optional heading over a card of rows. Each row
//! has its title (after an optional icon, over a muted detail line) at
//! the left and its control at the right; hairlines part the rows. Shapes
//! a control draws (a spinner, a check) go on the overlay plane, above the
//! card's quads, so the card is meant for a screen on the canvas plane.

use superfik_render::{Fill, Icon, ImageId, Point, Quad, Rect};

use super::status_button::{Progress, StatusButton};
use super::{Segmented, Switch, TextField};
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CardControl<'a> {
    Switch { on: bool, enabled: bool },
    Choice { labels: &'a [&'a str], selected: usize, enabled: bool },
    Button { labels: [&'a str; 3], icon: Icon, enabled: bool, progress: Progress },
    Buttons(&'a [(&'a str, Icon)]),
    Field(TextField<'a>),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Leading {
    Image(ImageId),
    Glyph(Icon),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardRow<'a> {
    pub leading: Option<Leading>,
    pub title: &'a str,
    pub detail: Option<&'a str>,
    pub control: CardControl<'a>,
}

impl CardRow<'_> {
    pub fn height(&self) -> f32 {
        if self.detail.is_some() { metrics::FORM_ROW_DETAILED } else { metrics::FORM_ROW }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FormCard<'a> {
    pub heading: Option<&'a str>,
    pub rows: &'a [CardRow<'a>],
    pub hovered: Option<(usize, usize)>,
}

impl FormCard<'_> {
    pub fn height(&self) -> f32 {
        let heading = if self.heading.is_some() { metrics::FORM_HEADING } else { 0.0 };
        heading + self.rows.iter().map(CardRow::height).sum::<f32>()
    }

    pub fn paint(&self, p: &mut Painter, area: Rect) -> Vec<Vec<Rect>> {
        let c = p.palette();
        let mut y = area.y;
        if let Some(heading) = self.heading {
            p.label(heading, typography::HEADER, area.x + 2.0, y + metrics::FORM_HEADING * 0.4, c.text_secondary);
            y += metrics::FORM_HEADING;
        }
        let card = Rect::new(area.x, y, area.w, self.height() - (y - area.y));
        p.quad(
            Quad::new(card, metrics::FORM_CARD_RADIUS, c.surface_top)
                .fill(Fill { top: c.surface_top, bottom: c.surface_bottom })
                .border(c.divider, 1.0),
        );
        let pad = metrics::FORM_ROW_PADDING;
        let mut hits = Vec::with_capacity(self.rows.len());
        for (i, row) in self.rows.iter().enumerate() {
            let rect = Rect::new(card.x, y, card.w, row.height());
            if i > 0 {
                p.quad(Quad::new(Rect::new(rect.x + pad, rect.y, rect.w - pad * 2.0, 1.0), 0.0, c.divider));
            }
            let hovered = self.hovered.filter(|(r, _)| *r == i).map(|(_, option)| option);
            let (control_x, hit) = paint_control(p, row.control, hovered, rect.right() - pad, rect.center().y);
            hits.push(hit);
            let mut x = rect.x + pad;
            if let Some(leading) = row.leading {
                let size = metrics::FORM_ICON;
                let slot = Rect::new(x, rect.center().y - size * 0.5, size, size);
                match leading {
                    Leading::Image(image) => p.image(image, slot, 0.0, 0.0),
                    Leading::Glyph(icon) => p.icon_at(icon, slot.center(), metrics::ICON_LARGE, Some(c.icon_muted)),
                }
                x += size + metrics::FORM_ICON_GAP;
            }
            let width = (control_x - metrics::FORM_COLUMN_GAP - x).max(0.0);
            match row.detail {
                Some(detail) => {
                    let title_y = rect.center().y - 9.0;
                    p.label_fit(row.title, typography::MENU, x, title_y, width, c.text);
                    p.label_fit(detail, typography::SUBTITLE, x, title_y + 18.0, width, c.text_secondary);
                }
                None => p.label_fit(row.title, typography::MENU, x, rect.center().y, width, c.text),
            }
            y += row.height();
        }
        hits
    }
}

fn paint_control(
    p: &mut Painter,
    control: CardControl<'_>,
    hovered: Option<usize>,
    right: f32,
    center_y: f32,
) -> (f32, Vec<Rect>) {
    match control {
        CardControl::Switch { on, enabled } => {
            let track = Switch { on, enabled }.paint(p, right, center_y);
            (track.x, if enabled { vec![track] } else { Vec::new() })
        }
        CardControl::Choice { labels, selected, enabled } => {
            let choice = Segmented { labels, selected, hovered: hovered.filter(|_| enabled) };
            let origin = Point::new(right - choice.width(p), (center_y - Segmented::HEIGHT * 0.5).round());
            if enabled {
                (origin.x, choice.paint(p, origin))
            } else {
                choice.paint_disabled(p, origin);
                (origin.x, Vec::new())
            }
        }
        CardControl::Button { labels, icon, enabled, progress } => {
            let button = StatusButton { labels, icon, progress, hovered: enabled && hovered.is_some(), enabled };
            let rect = button.paint(p, right, center_y);
            (rect.x, if enabled { vec![rect] } else { Vec::new() })
        }
        CardControl::Buttons(buttons) => {
            let mut right = right;
            let mut rects = vec![Rect::default(); buttons.len()];
            for (i, (label, icon)) in buttons.iter().enumerate().rev() {
                let labels = [*label; 3];
                let button = StatusButton {
                    labels,
                    icon: *icon,
                    progress: Progress::Idle,
                    hovered: hovered == Some(i),
                    enabled: true,
                };
                rects[i] = button.paint(p, right, center_y);
                right = rects[i].x - metrics::SETTINGS_BUTTON_GAP;
            }
            (right + metrics::SETTINGS_BUTTON_GAP, rects)
        }
        CardControl::Field(field) => {
            let (w, h) = (metrics::FORM_FIELD_WIDTH, metrics::FORM_FIELD_HEIGHT);
            let rect = Rect::new(right - w, (center_y - h * 0.5).round(), w, h);
            (rect.x, vec![field.paint(p, rect)])
        }
        CardControl::None => (right, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: [CardRow<'static>; 2] = [
        CardRow {
            leading: None,
            title: "Sounds",
            detail: None,
            control: CardControl::Switch { on: true, enabled: true },
        },
        CardRow {
            leading: None,
            title: "Pack",
            detail: Some("Which instrument"),
            control: CardControl::Choice { labels: &["Piano", "Sine"], selected: 0, enabled: true },
        },
    ];

    #[test]
    fn rows_with_a_detail_are_taller_and_the_heading_adds_its_line() {
        let bare = FormCard { heading: None, rows: &ROWS, hovered: None };
        assert_eq!(bare.height(), metrics::FORM_ROW + metrics::FORM_ROW_DETAILED);
        let headed = FormCard { heading: Some("Sound"), ..bare };
        assert_eq!(headed.height(), bare.height() + metrics::FORM_HEADING);
    }
}
