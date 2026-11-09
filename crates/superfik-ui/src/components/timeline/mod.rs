//! The session timeline: day groups on an outer rail, their sessions on an
//! inner rail, and a travelling dot marking the selected one.

mod rail;

pub use rail::{Piece, between, group_rail, x_at};

use superfik_render::{Color, Icon, Point, Quad, Rect};

use crate::icons;
use crate::painter::Painter;
use crate::text_edit::TextEdit;
use crate::theme::colors::Palette;
use crate::theme::{metrics, motion, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineItemProps<'a> {
    pub title: &'a str,
    pub running: bool,
    pub waiting: bool,
    pub selected: bool,
    pub archived: bool,
    pub editing: Option<(&'a TextEdit, bool)>,
    pub shortcut: Option<usize>,
    pub attention: Option<AttentionMark>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionMark {
    Asking,
    Failed,
    Done,
}

fn mark_glyph(palette: &Palette, mark: AttentionMark) -> (Icon, Color) {
    match mark {
        AttentionMark::Asking => (icons::QUESTION, palette.attention),
        AttentionMark::Failed => (icons::CLOSE_CIRCLE, palette.danger),
        AttentionMark::Done => (icons::CHECK_CIRCLE, palette.branch),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineGroupProps<'a> {
    pub label: &'a str,
    pub collapsed: bool,
    pub items: &'a [TimelineItemProps<'a>],
}

impl TimelineGroupProps<'_> {
    fn shown(&self) -> &[TimelineItemProps<'_>] {
        if self.collapsed { &[] } else { self.items }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineMarker {
    pub y: f32,
    pub trail_from: f32,
    pub trail: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineHit {
    New,
    Group(usize),
    Item { group: usize, item: usize },
    Archive { group: usize, item: usize },
}

impl TimelineHit {
    fn row(self) -> Self {
        match self {
            Self::Archive { group, item } => Self::Item { group, item },
            other => other,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Timeline<'a> {
    pub top_inset: f32,
    pub groups: &'a [TimelineGroupProps<'a>],
    pub hovered: Option<TimelineHit>,
    pub marker: Option<TimelineMarker>,
    pub scroll: f32,
    pub time: f32,
    pub new_label: &'a str,
    pub footer: f32,
    pub shortcuts: Option<&'a str>,
}

#[derive(Debug, Clone, Default)]
pub struct TimelineHits {
    pub rows: Vec<(Rect, TimelineHit)>,
    pub item_ys: Vec<(TimelineHit, f32)>,
    pub painted: Rect,
    pub content_height: f32,
}

impl Timeline<'_> {
    fn fitted_width(&self, p: &mut Painter) -> f32 {
        let label = metrics::TIMELINE_ITEM_X + metrics::TIMELINE_LABEL_GAP;
        let widest = self
            .groups
            .iter()
            .flat_map(|g| g.items.iter().map(|i| i.title).chain(std::iter::once(g.label)))
            .chain(std::iter::once(self.new_label))
            .map(|text| p.layout(text, typography::TIMELINE, None).width)
            .fold(0.0, f32::max);
        (label + widest + metrics::TIMELINE_TRAILING + metrics::TIMELINE_ROW_INSET)
            .ceil()
            .clamp(metrics::SIDEBAR_MIN_WIDTH, metrics::SIDEBAR_MAX_WIDTH)
    }

    pub fn paint(&self, p: &mut Painter, area: Rect) -> TimelineHits {
        let mut hits = TimelineHits::default();
        let width = self.fitted_width(p);
        hits.painted = Rect::new(area.x, area.y, width, area.h);
        let area = Rect::new(area.x, area.y, width, (area.h - self.footer).max(0.0));
        let rows_total = 1 + self.groups.iter().map(|g| 1 + g.shown().len()).sum::<usize>();
        let top = metrics::TIMELINE_TOP - metrics::TITLEBAR_HEIGHT + self.top_inset;
        hits.content_height = top + rows_total as f32 * metrics::TIMELINE_ROW + metrics::TIMELINE_BOTTOM;
        p.on_plane(crate::painter::Plane::Backdrop, |p| p.quad(Quad::new(hits.painted, 0.0, p.palette().canvas)));
        let row = |y: f32| {
            Rect::new(
                area.x + metrics::TIMELINE_ROW_INSET,
                y - metrics::TIMELINE_ROW_HEIGHT * 0.5,
                area.w - metrics::TIMELINE_ROW_INSET * 2.0,
                metrics::TIMELINE_ROW_HEIGHT,
            )
        };
        let mut y = area.y + top - self.scroll;
        let visible = |y: f32| y > area.y - metrics::TIMELINE_ROW && y < area.bottom() + metrics::TIMELINE_ROW;

        let new_row = row(y);
        if visible(y) {
            self.hover(p, new_row, self.hovered == Some(TimelineHit::New));
            let x = area.x + metrics::TIMELINE_GROUP_X;
            p.icon_at(icons::PLUS_CIRCLE, Point::new(x, y), metrics::ICON, Some(p.palette().icon_muted));
            p.label(
                self.new_label,
                typography::TIMELINE,
                x + metrics::TIMELINE_LABEL_GAP,
                y,
                p.palette().text_secondary,
            );
        }
        if visible(y)
            && let Some(key) = self.shortcuts
        {
            shortcut_hint(p, &format!("{key}N"), area.right() - metrics::TIMELINE_ROW_INSET - 4.0, y);
        }
        hits.rows.push((new_row, TimelineHit::New));
        y += metrics::TIMELINE_ROW;

        let mut group_ys = Vec::new();
        let mut item_ys: Vec<Vec<f32>> = Vec::new();
        for group in self.groups {
            group_ys.push(y);
            y += metrics::TIMELINE_ROW;
            let ys: Vec<f32> = group
                .shown()
                .iter()
                .map(|_| {
                    let iy = y;
                    y += metrics::TIMELINE_ROW;
                    iy
                })
                .collect();
            item_ys.push(ys);
        }
        let mut pieces = Vec::new();
        for (i, gy) in group_ys.iter().enumerate() {
            pieces.extend(group_rail(*gy, &item_ys[i], group_ys.get(i + 1).copied()));
        }
        let rail = |p: &mut Painter, piece: &Piece, color: Color| {
            let shape = piece.shape(metrics::TIMELINE_RAIL_RADIUS).translate(area.x, 0.0);
            p.well(shape, color, 0.0);
        };
        for piece in between(&pieces, area.y, area.bottom()) {
            rail(p, &piece, p.palette().timeline_rail);
        }
        if let Some(marker) = self.marker.filter(|m| m.trail > 0.0 && (m.trail_from - m.y).abs() > 0.5) {
            for piece in between(&pieces, marker.trail_from, marker.y) {
                rail(p, &piece, p.palette().timeline_accent.faded(marker.trail * 0.85));
            }
        }

        let dot = |p: &mut Painter, center: Point, radius: f32, color: Color| {
            p.quad(Quad::new(
                Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0),
                radius,
                color,
            ));
        };
        let pulse = ((self.time / motion::TIMELINE_PULSE) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        for (g, group) in self.groups.iter().enumerate() {
            let gy = group_ys[g];
            hits.rows.push((row(gy), TimelineHit::Group(g)));
            if visible(gy) {
                let hovered = self.hovered == Some(TimelineHit::Group(g));
                self.hover(p, row(gy), hovered);
                let x = area.x + metrics::TIMELINE_GROUP_X;
                dot(p, Point::new(x, gy), metrics::TIMELINE_DOT_RADIUS, p.palette().timeline_dot);
                let color = if hovered { p.palette().text_label } else { p.palette().text_secondary };
                let label = p.label(group.label, typography::TIMELINE, x + metrics::TIMELINE_LABEL_GAP, gy, color);
                if group.collapsed {
                    let count = group.items.len().to_string();
                    let count_x = x + metrics::TIMELINE_LABEL_GAP + label.width + 8.0;
                    p.label(&count, typography::TIMELINE, count_x, gy, p.palette().text_tertiary);
                }
            }
            for (i, item) in group.shown().iter().enumerate() {
                let iy = item_ys[g][i];
                let hit = TimelineHit::Item { group: g, item: i };
                hits.item_ys.push((hit, iy));
                hits.rows.push((row(iy), hit));
                if !visible(iy) {
                    continue;
                }
                let hovered = self.hovered.map(TimelineHit::row) == Some(hit);
                self.hover(p, row(iy), hovered);
                let x = area.x + metrics::TIMELINE_ITEM_X;
                let (dot_color, radius) = if item.waiting {
                    (p.palette().attention, metrics::TIMELINE_DOT_RADIUS + 1.0 + pulse)
                } else if item.running {
                    (p.palette().timeline_dot.mix(p.palette().text, pulse), metrics::TIMELINE_DOT_RADIUS + pulse)
                } else {
                    (p.palette().timeline_dot, metrics::TIMELINE_DOT_RADIUS)
                };
                dot(p, Point::new(x, iy), radius, dot_color);
                let color = if item.selected {
                    p.palette().text
                } else if hovered {
                    p.palette().text_label
                } else if item.running {
                    p.palette().text_secondary.mix(p.palette().text_label, pulse)
                } else if item.archived {
                    p.palette().text_tertiary
                } else {
                    p.palette().text_secondary
                };
                let label_x = x + metrics::TIMELINE_LABEL_GAP;
                let mut right = area.right() - metrics::TIMELINE_ROW_INSET - 4.0;
                if let (Some(digit), Some(key)) = (item.shortcut, self.shortcuts) {
                    let left = shortcut_hint(p, &format!("{key}{digit}"), right, iy);
                    right = left - metrics::TIMELINE_SHORTCUT_GAP;
                } else if let Some(mark) = item.attention.filter(|_| !hovered || item.editing.is_some()) {
                    let (glyph, tint) = mark_glyph(p.palette(), mark);
                    let glyph_x = right - metrics::TIMELINE_ROW_HEIGHT;
                    let center = Point::new(glyph_x + metrics::TIMELINE_ROW_HEIGHT * 0.5, iy);
                    p.icon_at(glyph, center, metrics::ICON, Some(tint));
                    right = glyph_x - 4.0;
                } else if hovered && item.editing.is_none() {
                    let archive = TimelineHit::Archive { group: g, item: i };
                    let glyph = Rect::new(
                        right - metrics::TIMELINE_ROW_HEIGHT,
                        iy - metrics::TIMELINE_ROW_HEIGHT * 0.5,
                        metrics::TIMELINE_ROW_HEIGHT,
                        metrics::TIMELINE_ROW_HEIGHT,
                    );
                    let tint = if self.hovered == Some(archive) { p.palette().icon } else { p.palette().icon_muted };
                    let icon = if item.archived { icons::ARROW_UP } else { icons::ARCHIVE };
                    p.icon_at(icon, glyph.center(), metrics::ICON, Some(tint));
                    hits.rows.push((glyph, archive));
                    right = glyph.x - 4.0;
                }
                match item.editing {
                    Some((edit, caret)) => {
                        title_editor(p, edit, caret, Rect::new(label_x - 6.0, iy - 12.0, right - label_x + 6.0, 24.0))
                    }
                    None => p.label_fit(item.title, typography::TIMELINE, label_x, iy, right - label_x, color),
                }
            }
        }
        if let Some(marker) = self.marker {
            let center = Point::new(area.x + x_at(&pieces, marker.y), marker.y);
            dot(p, center, metrics::TIMELINE_DOT_RADIUS + 0.5, p.palette().timeline_accent);
        }
        hits
    }

    fn hover(&self, p: &mut Painter, rect: Rect, hovered: bool) {
        if hovered {
            p.quad(Quad::new(rect, metrics::TIMELINE_ROW_RADIUS, p.palette().timeline_hover));
        }
    }
}

fn shortcut_hint(p: &mut Painter, text: &str, right: f32, y: f32) -> f32 {
    let width = p.layout(text, typography::TIMELINE, None).width;
    let x = right - width;
    p.label(text, typography::TIMELINE, x, y, p.palette().text_tertiary);
    x
}

fn title_editor(p: &mut Painter, edit: &TextEdit, caret_visible: bool, well: Rect) {
    p.quad(Quad::new(well, metrics::ROW_RADIUS, p.palette().well));
    let style = typography::TIMELINE;
    let origin = Point::new(well.x + 6.0, (well.center().y - style.line_height * 0.5).round());
    let layout = p.layout(edit.text(), style, None);
    if let Some((a, b)) = edit.selection()
        && let Some(line) = layout.lines.first()
    {
        let glyphs: Vec<_> = line.glyphs.iter().filter(|g| g.start >= a && g.end <= b).collect();
        if let (Some(first), Some(last)) = (glyphs.first(), glyphs.last()) {
            let rect = Rect::new(origin.x + first.x, origin.y, last.x + last.w - first.x, style.line_height);
            p.quad(Quad::new(rect, 2.0, p.palette().agent.faded(0.28)));
        }
    }
    p.text(&layout, origin, p.palette().text);
    if caret_visible {
        let x = layout
            .lines
            .first()
            .map(|line| line.glyphs.iter().filter(|g| g.start < edit.caret()).map(|g| g.x + g.w).fold(0.0, f32::max))
            .unwrap_or(0.0);
        p.quad(Quad::new(
            Rect::new(origin.x + x, origin.y + 2.0, 1.5, style.line_height - 4.0),
            0.75,
            p.palette().text,
        ));
    }
}

#[cfg(test)]
mod tests;
