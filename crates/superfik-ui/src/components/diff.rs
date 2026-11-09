//! A file's diff, stacked (one column, old and new numbers) or split (old
//! on the left, new on the right), with line selection and annotations
//! hanging under the lines they belong to. Ported from the `@pierre/diffs`
//! design: gutters, soft row tints, no borders.

use superfik_render::{Point, Quad, Rect};

use super::knob::Knob;
use super::sheet::Button;
use super::text_field::TextField;
use crate::icons;
use crate::painter::Painter;
use crate::text_edit::TextEdit;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffSide {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    pub kind: RowKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: u32,
    pub new_start: u32,
    pub heading: String,
    pub rows: Vec<DiffRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffLayout {
    #[default]
    Stacked,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineRef {
    pub side: DiffSide,
    pub line: u32,
}

impl LineRef {
    fn of(line: &DiffRow) -> Option<Self> {
        match (line.kind, line.old_no, line.new_no) {
            (RowKind::Removed, Some(n), _) => Some(Self { side: DiffSide::Old, line: n }),
            (_, _, Some(n)) => Some(Self { side: DiffSide::New, line: n }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKnob {
    Edit,
    Delete,
    Resolve,
    Reopen,
    Save,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
pub struct Annotation<'a> {
    pub at: LineRef,
    pub author: &'a str,
    pub badge: Option<&'a str>,
    pub body: &'a str,
    pub draft: bool,
    pub resolved: bool,
    pub editing: Option<&'a TextEdit>,
    pub hovered_knob: Option<AnnotationKnob>,
    pub caret_visible: bool,
    pub save_label: &'a str,
    pub cancel_label: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct DiffView<'a> {
    pub file: &'a DiffFile,
    pub layout: DiffLayout,
    pub selection: Option<(DiffSide, u32, u32)>,
    pub hovered: Option<LineRef>,
    pub annotations: &'a [Annotation<'a>],
    pub scroll_x: f32,
}

#[derive(Debug, Clone, Default)]
pub struct DiffHits {
    pub lines: Vec<(Rect, LineRef)>,
    pub knobs: Vec<(Rect, usize, AnnotationKnob)>,
    pub height: f32,
    pub overflow_x: f32,
}

pub const ROW: f32 = 20.0;
pub const HUNK_ROW: f32 = 26.0;
pub const FILE_HEADER: f32 = 34.0;
const GUTTER: f32 = 44.0;
const MARKER: f32 = 16.0;
const CARD_PADDING: f32 = 10.0;
const CARD_HEADER: f32 = 20.0;
const EDITOR_HEIGHT: f32 = 84.0;

pub fn split_rows(hunk: &DiffHunk) -> Vec<(Option<&DiffRow>, Option<&DiffRow>)> {
    let mut rows = Vec::new();
    let mut i = 0;
    let lines = &hunk.rows;
    while i < lines.len() {
        match lines[i].kind {
            RowKind::Context => {
                rows.push((Some(&lines[i]), Some(&lines[i])));
                i += 1;
            }
            RowKind::Removed | RowKind::Added => {
                let removed: Vec<&DiffRow> = lines[i..].iter().take_while(|l| l.kind == RowKind::Removed).collect();
                let added: Vec<&DiffRow> =
                    lines[i + removed.len()..].iter().take_while(|l| l.kind == RowKind::Added).collect();
                let count = removed.len().max(added.len()).max(1);
                for k in 0..count {
                    rows.push((removed.get(k).copied(), added.get(k).copied()));
                }
                i += (removed.len() + added.len()).max(1);
            }
        }
    }
    rows
}

impl DiffView<'_> {
    pub fn measure(&self, p: &mut Painter, width: f32) -> (f32, f32) {
        let mut hits = DiffHits::default();
        self.walk(None, &mut hits, Rect::new(0.0, 0.0, width, f32::MAX), p);
        (hits.height, hits.overflow_x)
    }

    pub fn height(&self, p: &mut Painter, width: f32) -> f32 {
        self.measure(p, width).0
    }

    fn note_overflow(p: &mut Painter, text: &str, offset: f32, available: f32, hits: &mut DiffHits) {
        let width = p.layout(text, typography::CODE, None).width;
        hits.overflow_x = hits.overflow_x.max(offset + width + 16.0 - available);
    }

    fn code(&self, p: &mut Painter, text: &str, text_x: f32, row: Rect, hits: &mut DiffHits) {
        let (cy, right) = (row.center().y, row.right() - 8.0);
        Self::note_overflow(p, text, text_x - row.x, row.w, hits);
        let clip = Rect::new(text_x, row.y, (right - text_x).max(0.0), row.h);
        p.clipped(clip, |p| {
            p.label(text, typography::CODE, text_x - self.scroll_x, cy, p.palette().text_body);
        });
    }

    pub fn paint(&self, p: &mut Painter, area: Rect) -> DiffHits {
        let mut hits = DiffHits::default();
        self.walk(Some(()), &mut hits, area, p);
        hits
    }

    fn walk(&self, paint: Option<()>, hits: &mut DiffHits, area: Rect, p: &mut Painter) {
        let paint = paint.is_some();
        let mut y = area.y;
        let header = Rect::new(area.x, y, area.w, FILE_HEADER);
        if paint {
            let counts = format!("+{} −{}", self.file.additions, self.file.deletions);
            p.label(&self.file.path, typography::MODEL, header.x + 12.0, header.center().y, p.palette().text);
            let w = p.layout(&counts, typography::CAPTION, None).width;
            p.label(
                &counts,
                typography::CAPTION,
                header.right() - 12.0 - w,
                header.center().y,
                p.palette().text_subtle,
            );
        }
        y += FILE_HEADER;
        for hunk in &self.file.hunks {
            let strip = Rect::new(area.x, y, area.w, HUNK_ROW);
            if paint {
                p.quad(Quad::new(strip, 0.0, p.palette().diff_hunk));
                let label = format!("@@ -{} +{} @@ {}", hunk.old_start, hunk.new_start, hunk.heading);
                p.label_fit(
                    &label,
                    typography::CODE_SMALL,
                    strip.x + 12.0,
                    strip.center().y,
                    strip.w - 24.0,
                    p.palette().text_subtle,
                );
            }
            y += HUNK_ROW;
            match self.layout {
                DiffLayout::Stacked => {
                    for line in &hunk.rows {
                        let row = Rect::new(area.x, y, area.w, ROW);
                        self.stacked_row(paint, p, row, line, hits);
                        y += ROW;
                        y += self.annotations_under(paint, p, LineRef::of(line), area, y, hits);
                    }
                }
                DiffLayout::Split => {
                    let half = area.w * 0.5;
                    for (left, right) in split_rows(hunk) {
                        let row_left = Rect::new(area.x, y, half, ROW);
                        let row_right = Rect::new(area.x + half, y, area.w - half, ROW);
                        self.split_cell(paint, p, row_left, left, DiffSide::Old, hits);
                        self.split_cell(paint, p, row_right, right, DiffSide::New, hits);
                        y += ROW;
                        let left_ref = left.filter(|l| l.kind == RowKind::Removed).and_then(LineRef::of);
                        let right_ref = right.and_then(LineRef::of).filter(|r| r.side == DiffSide::New);
                        y += self.annotations_under(paint, p, left_ref, area, y, hits);
                        y += self.annotations_under(paint, p, right_ref, area, y, hits);
                    }
                }
            }
        }
        hits.height = y - area.y + 12.0;
    }

    fn tint(&self, p: &Painter, line: &DiffRow, at: Option<LineRef>) -> Option<superfik_render::Color> {
        let c = p.palette();
        if let (Some(at), Some((side, from, to))) = (at, self.selection)
            && at.side == side
            && (from..=to).contains(&at.line)
        {
            return Some(c.diff_selected);
        }
        match line.kind {
            RowKind::Added => Some(c.diff_added),
            RowKind::Removed => Some(c.diff_removed),
            RowKind::Context => (at.is_some() && self.hovered == at).then_some(c.hover),
        }
    }

    fn stacked_row(&self, paint: bool, p: &mut Painter, row: Rect, line: &DiffRow, hits: &mut DiffHits) {
        let at = LineRef::of(line);
        if paint {
            if let Some(tint) = self.tint(p, line, at) {
                p.quad(Quad::new(row, 0.0, tint));
            }
            let cy = row.center().y;
            let numbers = p.palette().text_tertiary;
            if let Some(n) = line.old_no {
                number(p, &n.to_string(), row.x, GUTTER, cy, numbers);
            }
            if let Some(n) = line.new_no {
                number(p, &n.to_string(), row.x + GUTTER, GUTTER, cy, numbers);
            }
            let marker = match line.kind {
                RowKind::Added => "+",
                RowKind::Removed => "−",
                RowKind::Context => "",
            };
            let text_x = row.x + GUTTER * 2.0 + MARKER;
            p.label(marker, typography::CODE, row.x + GUTTER * 2.0 + 2.0, cy, p.palette().text_subtle);
            self.code(p, &line.text, text_x, row, hits);
        } else {
            Self::note_overflow(p, &line.text, GUTTER * 2.0 + MARKER, row.w, hits);
        }
        if let Some(at) = at {
            hits.lines.push((row, at));
        }
    }

    fn split_cell(
        &self,
        paint: bool,
        p: &mut Painter,
        cell: Rect,
        line: Option<&DiffRow>,
        side: DiffSide,
        hits: &mut DiffHits,
    ) {
        let Some(line) = line else { return };
        let at = match (line.kind, side) {
            (RowKind::Context, DiffSide::Old) => line.old_no.map(|n| LineRef { side, line: n }),
            _ => LineRef::of(line),
        };
        if paint {
            if let Some(tint) = self.tint(p, line, at) {
                p.quad(Quad::new(cell, 0.0, tint));
            }
            let cy = cell.center().y;
            let n = match side {
                DiffSide::Old => line.old_no,
                DiffSide::New => line.new_no,
            };
            if let Some(n) = n {
                number(p, &n.to_string(), cell.x, GUTTER, cy, p.palette().text_tertiary);
            }
            let text_x = cell.x + GUTTER + 6.0;
            self.code(p, &line.text, text_x, cell, hits);
        } else {
            Self::note_overflow(p, &line.text, GUTTER + 6.0, cell.w, hits);
        }
        if let Some(at) = at {
            hits.lines.push((cell, at));
        }
    }

    fn annotations_under(
        &self,
        paint: bool,
        p: &mut Painter,
        at: Option<LineRef>,
        area: Rect,
        y: f32,
        hits: &mut DiffHits,
    ) -> f32 {
        let Some(at) = at else { return 0.0 };
        let mut taken = 0.0;
        for (index, annotation) in self.annotations.iter().enumerate() {
            if annotation.at != at {
                continue;
            }
            let card_x = area.x + GUTTER;
            let card_w = area.w - GUTTER - 12.0;
            let body_w = card_w - CARD_PADDING * 2.0;
            let inner = match annotation.editing {
                Some(_) => EDITOR_HEIGHT + 6.0 + metrics::CHIP_HEIGHT,
                None => p.layout(annotation.body, typography::MENU, Some(body_w)).height,
            };
            let height = CARD_PADDING + CARD_HEADER + inner + CARD_PADDING;
            let card = Rect::new(card_x, y + taken + 4.0, card_w, height);
            if paint {
                p.quad(Quad::new(card, metrics::WELL_RADIUS, p.palette().well));
                let header_y = card.y + CARD_PADDING + CARD_HEADER * 0.5;
                let author = if annotation.author.is_empty() { "You" } else { annotation.author };
                let layout = p.label(author, typography::TITLE, card.x + CARD_PADDING, header_y, p.palette().text);
                if let Some(badge) = annotation.badge {
                    let badge = super::badge::Badge { text: badge };
                    let w = badge.width(p);
                    badge.paint(p, card.x + CARD_PADDING + layout.width + 8.0 + w, header_y);
                }
                match annotation.editing {
                    Some(edit) => {
                        let field = Rect::new(
                            card.x + CARD_PADDING,
                            card.y + CARD_PADDING + CARD_HEADER,
                            body_w,
                            EDITOR_HEIGHT,
                        );
                        TextField {
                            edit,
                            placeholder: "",
                            focused: true,
                            caret_visible: annotation.caret_visible,
                            multiline: true,
                            secret: false,
                        }
                        .paint(p, field);
                        let save = Button {
                            label: annotation.save_label,
                            primary: true,
                            hovered: annotation.hovered_knob == Some(AnnotationKnob::Save),
                            enabled: !edit.is_empty(),
                        };
                        let w = save.width(p);
                        let rect =
                            save.paint(p, Rect::new(field.right() - w, field.bottom() + 6.0, w, metrics::CHIP_HEIGHT));
                        hits.knobs.push((rect, index, AnnotationKnob::Save));
                        let cancel = Button {
                            label: annotation.cancel_label,
                            primary: false,
                            hovered: annotation.hovered_knob == Some(AnnotationKnob::Cancel),
                            enabled: true,
                        };
                        let w = cancel.width(p);
                        let rect =
                            cancel.paint(p, Rect::new(rect.x - 8.0 - w, field.bottom() + 6.0, w, metrics::CHIP_HEIGHT));
                        hits.knobs.push((rect, index, AnnotationKnob::Cancel));
                    }
                    None => {
                        let layout = p.layout(annotation.body, typography::MENU, Some(body_w));
                        let color =
                            if annotation.resolved { p.palette().text_secondary } else { p.palette().text_body };
                        p.text(&layout, Point::new(card.x + CARD_PADDING, card.y + CARD_PADDING + CARD_HEADER), color);
                        let mut x = card.right() - CARD_PADDING - Knob::RADIUS;
                        let knobs: &[(AnnotationKnob, superfik_render::Icon)] = if annotation.draft {
                            &[(AnnotationKnob::Delete, icons::CLOSE), (AnnotationKnob::Edit, icons::PENCIL)]
                        } else if annotation.resolved {
                            &[(AnnotationKnob::Reopen, icons::ARROW_UP)]
                        } else {
                            &[(AnnotationKnob::Resolve, icons::CHECK_CIRCLE)]
                        };
                        for (knob, icon) in knobs {
                            let hovered = annotation.hovered_knob == Some(*knob);
                            let center = Point::new(x, header_y);
                            let tint = if hovered { p.palette().text } else { p.palette().icon_muted };
                            p.icon_at(*icon, center, 13.0, Some(tint));
                            let rect = Rect::new(center.x - 10.0, center.y - 10.0, 20.0, 20.0);
                            hits.knobs.push((rect, index, *knob));
                            x -= 24.0;
                        }
                    }
                }
            } else if annotation.editing.is_some() {
            }
            taken += height + 4.0;
        }
        if taken > 0.0 { taken + 4.0 } else { 0.0 }
    }
}

fn number(p: &mut Painter, text: &str, x: f32, width: f32, cy: f32, color: superfik_render::Color) {
    let w = p.layout(text, typography::CODE_SMALL, None).width;
    p.label(text, typography::CODE_SMALL, x + width - 8.0 - w, cy, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hunk(spec: &str) -> DiffHunk {
        let mut lines = Vec::new();
        let (mut old, mut new) = (1, 1);
        for c in spec.chars() {
            let (kind, old_no, new_no) = match c {
                '-' => (RowKind::Removed, Some(old), None),
                '+' => (RowKind::Added, None, Some(new)),
                _ => (RowKind::Context, Some(old), Some(new)),
            };
            old += u32::from(old_no.is_some());
            new += u32::from(new_no.is_some());
            lines.push(DiffRow { kind, old_no, new_no, text: c.to_string() });
        }
        DiffHunk { old_start: 1, new_start: 1, heading: String::new(), rows: lines }
    }

    #[test]
    fn split_rows_pair_removed_with_added() {
        let h = hunk(" --+ +");
        let rows = split_rows(&h);
        let shape: Vec<(Option<RowKind>, Option<RowKind>)> =
            rows.iter().map(|(l, r)| (l.map(|l| l.kind), r.map(|r| r.kind))).collect();
        assert_eq!(
            shape,
            vec![
                (Some(RowKind::Context), Some(RowKind::Context)),
                (Some(RowKind::Removed), Some(RowKind::Added)),
                (Some(RowKind::Removed), None),
                (Some(RowKind::Context), Some(RowKind::Context)),
                (None, Some(RowKind::Added)),
            ]
        );
    }

    #[test]
    fn line_refs_follow_the_side_that_has_the_line() {
        let h = hunk("-+ ");
        assert_eq!(LineRef::of(&h.rows[0]), Some(LineRef { side: DiffSide::Old, line: 1 }));
        assert_eq!(LineRef::of(&h.rows[1]), Some(LineRef { side: DiffSide::New, line: 1 }));
        assert_eq!(LineRef::of(&h.rows[2]), Some(LineRef { side: DiffSide::New, line: 2 }));
    }
}
