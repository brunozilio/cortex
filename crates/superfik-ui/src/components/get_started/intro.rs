//! Get Started's intro, on the night stage: the greeting resolving word by
//! word, then the cast (coffee, the emote, a person, a loop) popping in on
//! a row, joined by trails of dots that light up in turn. Painted with the
//! dark palette in either theme. The emote itself is painted by the caller,
//! which flies it to the cards.

use superfik_render::{Color, Icon, Point, Quad, Rect};

use super::choreography::Beat;
use crate::icons;
use crate::painter::Painter;
use crate::theme::{colors, metrics, motion, typography};

pub const CAST: [Option<Icon>; 4] = [Some(icons::COFFEE), None, Some(icons::PERSON), Some(icons::LOOP)];
pub const EMOTE_AT: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stage {
    pub greeting_y: f32,
    pub cast_y: f32,
    pub center_x: f32,
}

impl Stage {
    pub fn new(area: Rect) -> Self {
        let height = typography::GET_STARTED_WELCOME.line_height
            + metrics::GET_STARTED_CAST_GAP
            + metrics::GET_STARTED_CAST_EMOTE;
        let greeting_y = area.center().y - height * 0.5;
        let cast_y = greeting_y
            + typography::GET_STARTED_WELCOME.line_height
            + metrics::GET_STARTED_CAST_GAP
            + metrics::GET_STARTED_CAST_EMOTE * 0.5;
        Self { greeting_y, cast_y, center_x: area.center().x }
    }

    pub fn member(&self, i: usize) -> Point {
        let offset = (i as f32 - (CAST.len() - 1) as f32 * 0.5) * metrics::GET_STARTED_CAST_SPACING;
        Point::new(self.center_x + offset, self.cast_y)
    }

    fn reach(i: usize) -> f32 {
        if i == EMOTE_AT { metrics::GET_STARTED_CAST_EMOTE * 0.5 } else { metrics::GET_STARTED_CAST_GLYPH * 0.5 }
    }
}

pub fn paint(p: &mut Painter, stage: &Stage, welcome: &str, beat: Beat) {
    let leave = beat.leave();
    if leave >= 1.0 {
        return;
    }
    p.with_palette(&colors::DARK, |p| {
        let away = leave * metrics::GET_STARTED_RISE * 2.0;
        p.faded(1.0 - leave, |p| {
            p.blurred(leave * motion::BLUR, |p| {
                paint_greeting(p, stage, welcome, beat, away);
                for i in 0..CAST.len() - 1 {
                    paint_trail(p, stage, i, beat, away);
                }
                for (i, icon) in CAST.iter().enumerate() {
                    if let Some(icon) = icon {
                        paint_glyph(p, *icon, tint(p, i), stage.member(i).y - away, stage.member(i).x, beat.cast(i));
                    }
                }
            });
        });
    });
}

fn tint(p: &Painter, i: usize) -> Color {
    let c = p.palette();
    match i {
        0 => c.memory,
        3 => c.agent,
        _ => c.text,
    }
}

fn paint_greeting(p: &mut Painter, stage: &Stage, welcome: &str, beat: Beat, away: f32) {
    let style = typography::GET_STARTED_WELCOME;
    let whole = p.layout(welcome, style, None);
    let space = p.layout("a a", style, None).width - p.layout("aa", style, None).width;
    let mut x = (stage.center_x - whole.width * 0.5).round();
    for (i, word) in welcome.split_whitespace().enumerate() {
        let layout = p.layout(word, style, None);
        let t = beat.word(i);
        if t > 0.0 {
            let left = 1.0 - t;
            let y = stage.greeting_y + left * metrics::GET_STARTED_RISE - away;
            let ink = p.palette().text;
            p.faded(t, |p| p.blurred(left * motion::BLUR * 2.0, |p| p.text(&layout, Point::new(x, y), ink)));
        }
        x += layout.width + space;
    }
}

fn paint_glyph(p: &mut Painter, icon: Icon, tint: Color, y: f32, x: f32, t: f32) {
    if t <= 0.0 {
        return;
    }
    let from = metrics::GET_STARTED_CAST_FROM;
    let size = metrics::GET_STARTED_CAST_GLYPH * (from + (1.0 - from) * t);
    p.faded(t, |p| p.blurred((1.0 - t) * motion::BLUR, |p| p.icon_at(icon, Point::new(x, y), size, Some(tint))));
}

fn paint_trail(p: &mut Painter, stage: &Stage, i: usize, beat: Beat, away: f32) {
    let drawn = beat.trail(i);
    if drawn <= 0.0 {
        return;
    }
    let gap = metrics::GET_STARTED_CAST_GLYPH * 0.4;
    let (a, b) = (stage.member(i), stage.member(i + 1));
    let start = a.x + Stage::reach(i) + gap;
    let end = b.x - Stage::reach(i + 1) - gap;
    let n = metrics::GET_STARTED_TRAIL_DOTS;
    let total = (n * (CAST.len() - 1)) as f32;
    let r = metrics::GET_STARTED_TRAIL_DOT;
    let ink = p.palette().text_secondary;
    for k in 0..n {
        let shown = (drawn * n as f32 - k as f32).clamp(0.0, 1.0);
        if shown <= 0.0 {
            continue;
        }
        let along = (k as f32 + 0.5) / n as f32;
        let phase = beat.0 / motion::GET_STARTED_TRAIL_FLOW - (i * n + k) as f32 / total;
        let wave = 0.5 + 0.5 * (phase * std::f32::consts::TAU).cos();
        let center = Point::new(start + (end - start) * along, a.y - away);
        let rect = Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0);
        p.quad(Quad::new(rect, r, ink.faded(shown * (0.3 + 0.7 * wave * wave * wave))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cast_is_centred_with_the_emote_second() {
        let stage = Stage::new(Rect::new(0.0, 0.0, 1200.0, 800.0));
        let (first, last) = (stage.member(0), stage.member(CAST.len() - 1));
        assert!(((first.x + last.x) * 0.5 - 600.0).abs() < 0.01);
        assert!(CAST[EMOTE_AT].is_none(), "the emote is painted by the caller");
        assert!(stage.greeting_y < stage.cast_y && stage.cast_y < 800.0);
    }
}
