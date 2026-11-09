//! Skills and accounts named in the composer, drawn as highlights: a
//! tinted pill behind the token, its text in the accent, and the `/` or `#`
//! given up for the skill's glyph or the account's brand tile. The text is
//! laid out with that character swapped for a wide letter of the same byte length,
//! so carets and selections keep their byte offsets, and with extra room
//! after the mark and in the spaces around the token for the pill.

use superfik_render::{Color, Icon, Point, Quad, Rect, TextLayout};

use crate::brands::Brand;
use crate::components::brand_tile::BrandTile;
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HighlightMark {
    Icon(Icon, Color),
    Brand(Brand),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    pub start: usize,
    pub end: usize,
    pub color: Color,
    pub mark: HighlightMark,
}

pub(super) fn display_text(text: &str, highlights: &[Highlight]) -> String {
    let mut bytes = text.as_bytes().to_vec();
    for h in highlights {
        if matches!(bytes.get(h.start), Some(b'/' | b'#')) {
            bytes[h.start] = b'M';
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_owned())
}

pub(super) fn spacing(text: &str, highlights: &[Highlight]) -> Vec<(usize, f32)> {
    let em = |px: f32| px / typography::INPUT.size;
    let mut spaced = Vec::new();
    for h in highlights {
        if let Some(before) = text.get(..h.start).and_then(|t| t.char_indices().next_back())
            && is_gap(before.1)
        {
            spaced.push((before.0, em(metrics::TOKEN_SPACE)));
        }
        spaced.push((h.start, em(metrics::TOKEN_MARK_GAP)));
        if text.get(h.end..).and_then(|t| t.chars().next()).is_some_and(is_gap) {
            spaced.push((h.end, em(metrics::TOKEN_SPACE)));
        }
    }
    spaced.sort_by_key(|s| s.0);
    spaced.dedup_by_key(|s| s.0);
    spaced
}

fn is_gap(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\n' | '\r')
}

pub(super) fn covering(highlights: &[Highlight], at: usize) -> Option<&Highlight> {
    highlights.iter().find(|h| at >= h.start && at < h.end)
}

pub(super) fn paint_pills(
    p: &mut Painter,
    layout: &TextLayout,
    origin: Point,
    line_height: f32,
    highlights: &[Highlight],
) {
    for h in highlights {
        for line in &layout.lines {
            let glyphs: Vec<_> = line.glyphs.iter().filter(|g| g.start >= h.start && g.end <= h.end).collect();
            let (Some(first), Some(last)) = (glyphs.first(), glyphs.last()) else { continue };
            let pad = metrics::TOKEN_PAD;
            let rect = Rect::new(
                origin.x + first.x - pad,
                origin.y + line.top + 1.0,
                last.x + last.w - first.x + pad * 2.0,
                line_height - 2.0,
            );
            p.quad(Quad::new(rect, metrics::TOKEN_RADIUS, h.color.faded(metrics::TOKEN_FILL)));
        }
    }
}

pub(super) fn paint_marks(p: &mut Painter, layout: &TextLayout, origin: Point, highlights: &[Highlight]) {
    for h in highlights {
        for line in &layout.lines {
            let Some(slot) = line.glyphs.iter().find(|g| g.start == h.start) else { continue };
            let size = metrics::TOKEN_MARK;
            let rise = typography::INPUT.size * metrics::TOKEN_MARK_RISE;
            let center = Point::new(origin.x + slot.x + size * 0.5, origin.y + line.baseline - rise);
            match h.mark {
                HighlightMark::Icon(icon, tint) => p.icon_at(icon, center, size, Some(tint)),
                HighlightMark::Brand(brand) => {
                    let rect = Rect::new(center.x - size * 0.5, center.y - size * 0.5, size, size);
                    BrandTile { brand, name: "" }.paint(p, rect);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slashes_give_way_without_moving_bytes() {
        let h = Highlight {
            start: 4,
            end: 11,
            color: Color::hex(0xffffff),
            mark: HighlightMark::Icon(crate::icons::BOLT, Color::hex(0xffffff)),
        };
        let shown = display_text("fix /motion now", &[h]);
        assert_eq!(shown, "fix Mmotion now");
        assert_eq!(shown.len(), "fix /motion now".len());
        assert!(covering(&[h], 4).is_some() && covering(&[h], 11).is_none());
    }

    #[test]
    fn the_spaces_around_a_token_and_its_mark_get_room() {
        let h = |start, end| Highlight {
            start,
            end,
            color: Color::hex(0xffffff),
            mark: HighlightMark::Icon(crate::icons::BOLT, Color::hex(0xffffff)),
        };
        let at: Vec<usize> = spacing("fix /motion now", &[h(4, 11)]).into_iter().map(|s| s.0).collect();
        assert_eq!(at, [3, 4, 11]);
        let at: Vec<usize> = spacing("/a /b", &[h(0, 2), h(3, 5)]).into_iter().map(|s| s.0).collect();
        assert_eq!(at, [0, 2, 3], "a shared space is widened once, nothing outside the text");
    }
}

#[cfg(test)]
mod layout_tests {
    use superfik_render::{Flow, TextSystem};

    use super::*;

    #[test]
    fn a_newline_after_a_token_opens_a_line_at_once() {
        let mut text = TextSystem::new();
        let h = Highlight {
            start: 0,
            end: 7,
            color: Color::hex(0xffffff),
            mark: HighlightMark::Icon(crate::icons::BOLT, Color::hex(0xffffff)),
        };
        let source = "/motion\n";
        let shown = display_text(source, &[h]);
        let layout = text.layout_spaced(&shown, typography::INPUT, Flow::plain(400.0), &spacing(source, &[h]));
        let (_, top) = crate::components::text_field::caret_position(&layout, source.len());
        assert_eq!(top, typography::INPUT.line_height, "caret drops to the second line");
    }
}
