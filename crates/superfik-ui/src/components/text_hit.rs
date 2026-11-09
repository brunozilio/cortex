//! Where text is: the byte offset under a point and the one a line above or
//! below the caret. Pure geometry over a shaped layout, for the pointer and
//! the arrow keys of a multi-line field.

use superfik_render::TextLayout;

use super::text_field::caret_position;

pub fn offset_at(layout: &TextLayout, x: f32, y: f32, line_height: f32) -> usize {
    let Some(line) = layout.lines.iter().find(|l| y < l.top + line_height).or(layout.lines.last()) else {
        return 0;
    };
    for glyph in &line.glyphs {
        if x < glyph.x + glyph.w * 0.5 {
            return glyph.start;
        }
    }
    line.glyphs.last().map_or(line.start, |g| g.end)
}

pub fn step_line(layout: &TextLayout, caret: usize, down: bool, line_height: f32) -> Option<usize> {
    let (x, top) = caret_position(layout, caret);
    let at = layout.lines.iter().position(|l| (l.top - top).abs() < 0.5)?;
    let next = if down { at.checked_add(1)? } else { at.checked_sub(1)? };
    let line = layout.lines.get(next)?;
    Some(offset_at(layout, x, line.top + line_height * 0.5, line_height))
}

#[cfg(test)]
mod tests {
    use superfik_render::TextSystem;

    use super::*;
    use crate::theme::typography;

    const LINE: f32 = typography::INPUT.line_height;

    fn layout(text: &str) -> std::sync::Arc<TextLayout> {
        TextSystem::new().layout(text, typography::INPUT, Some(400.0))
    }

    #[test]
    fn a_point_picks_the_nearest_boundary() {
        let l = layout("ab\ncd");
        assert_eq!(offset_at(&l, 0.0, 5.0, LINE), 0);
        assert_eq!(offset_at(&l, 300.0, 5.0, LINE), 2, "past the end of a line");
        assert_eq!(offset_at(&l, 0.0, LINE + 5.0, LINE), 3);
        assert_eq!(offset_at(&l, 0.0, 900.0, LINE), 3, "below the text: the last line");
    }

    #[test]
    fn arrows_walk_the_lines_and_stop_at_the_ends() {
        let l = layout("abc\nde\nfgh");
        assert_eq!(step_line(&l, 1, true, LINE), Some(5), "a|bc → d|e... clamps to the shorter line");
        assert_eq!(step_line(&l, 8, false, LINE), Some(5));
        assert_eq!(step_line(&l, 1, false, LINE), None, "first line");
        assert_eq!(step_line(&l, 9, true, LINE), None, "last line");
    }

    #[test]
    fn empty_lines_take_the_caret() {
        let l = layout("a\n\nb");
        assert_eq!(step_line(&l, 1, true, LINE), Some(2));
        assert_eq!(step_line(&l, 2, true, LINE), Some(3));
    }
}
