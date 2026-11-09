//! A pipe table inside a reply: a panel, a header row, rules between rows.
//! Columns take the room their text wants and share what is short.

use std::sync::Arc;

use superfik_render::{Flow, Point, Quad, Rect, TextLayout, TextStyle};

use super::reply::Reveal;
use crate::markdown::{self, Clean, Kind};
use crate::painter::Painter;
use crate::theme::{metrics, motion, typography};

pub(super) struct Table<'a> {
    pub header: &'a [String],
    pub rows: &'a [Vec<String>],
    pub width: f32,
}

struct Cell {
    layout: Arc<TextLayout>,
    clean: Clean,
}

fn column_widths(natural: &[f32], room: f32) -> Vec<f32> {
    let total: f32 = natural.iter().sum();
    if total <= room {
        return natural.to_vec();
    }
    let min = metrics::TABLE_MIN_COLUMN.min(room / natural.len().max(1) as f32);
    natural.iter().map(|w| (w * room / total).max(min)).collect()
}

impl Table<'_> {
    fn cell(p: &mut Painter, text: &str, style: TextStyle, width: f32) -> Cell {
        let clean = markdown::clean(text);
        let layout = p.layout_styled(&clean.text, style, Flow::plain(width), &[], &clean.runs());
        Cell { layout, clean }
    }

    fn lay(&self, p: &mut Painter) -> (Vec<f32>, Vec<Vec<Cell>>) {
        let pad = metrics::TABLE_PAD_X * 2.0;
        let rows = std::iter::once((self.header, typography::TABLE_HEAD))
            .chain(self.rows.iter().map(|r| (r.as_slice(), typography::TABLE)));
        let rows: Vec<(&[String], TextStyle)> = rows.collect();
        let natural: Vec<f32> = (0..self.header.len())
            .map(|i| {
                rows.iter()
                    .filter_map(|(row, style)| row.get(i).map(|t| (t, *style)))
                    .map(|(t, style)| {
                        let clean = markdown::clean(t);
                        p.layout_styled(&clean.text, style, Flow::plain(f32::MAX / 4.0), &[], &clean.runs()).width
                    })
                    .fold(0.0, f32::max)
                    + pad
            })
            .collect();
        let columns = column_widths(&natural, self.width);
        let cells = rows
            .iter()
            .map(|(row, style)| {
                columns
                    .iter()
                    .enumerate()
                    .map(|(i, w)| Self::cell(p, row.get(i).map_or("", String::as_str), *style, (w - pad).max(1.0)))
                    .collect()
            })
            .collect();
        (columns, cells)
    }

    fn row_height(row: &[Cell]) -> f32 {
        row.iter().map(|c| c.layout.height).fold(0.0, f32::max) + metrics::TABLE_PAD_Y * 2.0
    }

    pub(super) fn height(&self, p: &mut Painter) -> f32 {
        self.lay(p).1.iter().map(|row| Self::row_height(row)).sum()
    }

    pub(super) fn paint(&self, p: &mut Painter, origin: Point, reveal: Reveal, words_before: usize) -> (f32, usize) {
        let (columns, rows) = self.lay(p);
        let c = p.palette();
        let height: f32 = rows.iter().map(|row| Self::row_height(row)).sum();
        let first = reveal(words_before);
        p.quad(Quad::new(
            Rect::new(origin.x, origin.y, self.width, height),
            metrics::ROW_RADIUS,
            c.text_body.faded(metrics::CODE_BLOCK_FILL * first),
        ));
        let mut y = origin.y;
        let mut words = words_before;
        for (n, row) in rows.iter().enumerate() {
            let h = Self::row_height(row);
            if n > 0 {
                let rule = Rect::new(origin.x, y, self.width, 1.0);
                p.quad(Quad::new(rule, 0.0, c.text_body.faded(metrics::TABLE_RULE * reveal(words))));
            }
            let mut x = origin.x;
            for (cell, width) in row.iter().zip(&columns) {
                let base = words;
                let at = Point::new(x + metrics::TABLE_PAD_X, y + metrics::TABLE_PAD_Y);
                p.text_with(&cell.layout, at, |offset| {
                    let t = reveal(base + cell.layout.word_index(offset));
                    let color = match cell.clean.kind_at(offset) {
                        Some(Kind::Code) => c.text_code,
                        Some(Kind::Link) => c.form_accent,
                        _ => c.text_body,
                    };
                    (color.faded(t), (1.0 - t) * motion::BLUR)
                });
                words += cell.clean.text.split_whitespace().count();
                x += width;
            }
            y += h;
        }
        (height, words - words_before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_keep_their_width_when_they_fit() {
        assert_eq!(column_widths(&[80.0, 120.0], 300.0), [80.0, 120.0]);
    }

    #[test]
    fn wide_tables_shrink_in_proportion_but_not_below_the_minimum() {
        let widths = column_widths(&[200.0, 600.0, 20.0], 400.0);
        assert!(widths[1] > widths[0], "{widths:?}");
        assert_eq!(widths[2], metrics::TABLE_MIN_COLUMN);
    }
}
