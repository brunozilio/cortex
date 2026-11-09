//! The changed files of a pull request, one row each: status glyph, path
//! (fitted from the left so the file name stays visible), +/- counts and
//! a comment count.

use superfik_render::{Quad, Rect};

use super::badge::Badge;
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileGlyph {
    Added,
    Removed,
    Modified,
    Renamed,
}

impl FileGlyph {
    fn letter(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Removed => "D",
            Self::Modified => "M",
            Self::Renamed => "R",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileRow<'a> {
    pub path: &'a str,
    pub glyph: FileGlyph,
    pub additions: usize,
    pub deletions: usize,
    pub comments: usize,
    pub selected: bool,
    pub hovered: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct FileList<'a> {
    pub rows: &'a [FileRow<'a>],
}

impl FileList<'_> {
    pub const ROW: f32 = 28.0;

    pub fn height(count: usize) -> f32 {
        count as f32 * Self::ROW
    }

    pub fn paint(&self, p: &mut Painter, area: Rect) -> Vec<Rect> {
        let mut rects = Vec::with_capacity(self.rows.len());
        for (i, row) in self.rows.iter().enumerate() {
            let rect = Rect::new(area.x, area.y + i as f32 * Self::ROW, area.w, Self::ROW);
            if row.selected || row.hovered {
                let fill = if row.selected { p.palette().control_active } else { p.palette().hover };
                p.quad(Quad::new(rect, metrics::ROW_RADIUS, fill));
            }
            let cy = rect.center().y;
            let glyph_color = match row.glyph {
                FileGlyph::Added => p.palette().branch,
                FileGlyph::Removed => p.palette().danger,
                FileGlyph::Modified => p.palette().research,
                FileGlyph::Renamed => p.palette().skill,
            };
            p.label(row.glyph.letter(), typography::LABEL, rect.x + 8.0, cy, glyph_color);
            let counts = format!("+{} −{}", row.additions, row.deletions);
            let counts_width = p.layout(&counts, typography::CAPTION, None).width;
            let mut right = rect.right() - 8.0;
            if row.comments > 0 {
                let text = row.comments.to_string();
                let badge = Badge { text: &text }.paint(p, right, cy);
                right -= badge.w + 6.0;
            }
            p.label(&counts, typography::CAPTION, right - counts_width, cy, p.palette().text_subtle);
            let color = if row.selected { p.palette().text } else { p.palette().text_label };
            let name_x = rect.x + 24.0;
            let max = (right - counts_width - 8.0 - name_x).max(0.0);
            let fitted = fit_path(p, row.path, max);
            p.label_fit_for(&fitted, row.path, typography::LABEL, (name_x, cy), max, color);
            rects.push(rect);
        }
        rects
    }
}

fn fit_path(p: &mut Painter, path: &str, max: f32) -> String {
    if p.layout(path, typography::LABEL, None).width <= max {
        return path.to_owned();
    }
    let parts: Vec<&str> = path.split('/').collect();
    for skip in 1..parts.len() {
        let candidate = format!("…/{}", parts[skip..].join("/"));
        if p.layout(&candidate, typography::LABEL, None).width <= max {
            return candidate;
        }
    }
    parts.last().map_or_else(|| path.to_owned(), |name| (*name).to_owned())
}
