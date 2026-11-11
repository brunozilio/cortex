//! "<command>" in big letters (`tui-big-text`, a quadrant per font pixel),
//! coloured from magenta to blue across its width, as lines the
//! conversation starts with.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use tui_big_text::{BigText, PixelSize};

const WORD: &str = brand::COMMAND;
/// Columns and rows the word takes: four columns and four rows a letter.
const WIDTH: u16 = 4 * WORD.len() as u16;
const HEIGHT: u16 = 4;
const FROM: (u8, u8, u8) = (198, 120, 221);
const TO: (u8, u8, u8) = (97, 175, 239);

/// The logo's lines, or `None` when `width` columns cannot hold it.
pub fn lines(width: usize) -> Option<Vec<Line<'static>>> {
    if width < usize::from(WIDTH) + 2 {
        return None;
    }
    let area = Rect::new(0, 0, WIDTH, HEIGHT);
    let mut buffer = Buffer::empty(area);
    BigText::builder().pixel_size(PixelSize::Quadrant).lines(vec![WORD.into()]).build().render(area, &mut buffer);
    let lines = (0..HEIGHT)
        .map(|y| {
            let mut spans = vec![Span::raw(" ")];
            spans.extend(
                (0..WIDTH).map(|x| Span::styled(buffer[(x, y)].symbol().to_owned(), Style::new().fg(shade(x)))),
            );
            Line::from(spans)
        })
        .collect();
    Some(lines)
}

/// The colour at column `x`, between [`FROM`] and [`TO`].
fn shade(x: u16) -> Color {
    let t = f32::from(x) / f32::from(WIDTH - 1);
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    Color::Rgb(mix(FROM.0, TO.0), mix(FROM.1, TO.1), mix(FROM.2, TO.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_fills_four_rows_when_it_fits_and_steps_aside_when_not() {
        let logo = lines(80).expect("fits");
        assert_eq!(logo.len(), 4);
        assert!(logo.iter().any(|l| l.spans.iter().any(|s| !s.content.trim().is_empty())), "something is drawn");
        assert_eq!(logo[0].spans[1].style.fg, Some(Color::Rgb(198, 120, 221)));
        assert_eq!(logo[0].spans.last().and_then(|s| s.style.fg), Some(Color::Rgb(97, 175, 239)));
        assert!(lines(20).is_none());
    }
}
