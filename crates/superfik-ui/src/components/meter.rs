//! Effort label + dithered meter shown in the composer header.

use superfik_render::{Color, Pattern, Point, Quad, Rect};

use crate::painter::Painter;
use crate::theme::{colors, motion, typography};

#[derive(Debug, Clone, Copy)]
pub struct EffortMeter<'a> {
    pub label: &'a str,
    pub level: f32,
    pub ultra: bool,
    pub hovered: bool,
    pub time: f32,
}

fn ultra_hue(time: f32) -> f32 {
    (colors::ULTRA_HUE + time / motion::ULTRA_CYCLE).rem_euclid(1.0)
}

impl EffortMeter<'_> {
    pub const BOX: (f32, f32) = (107.0, 26.0);

    fn frame(right: f32, cy: f32) -> Rect {
        let (w, h) = Self::BOX;
        Rect::new(right - w, cy - h * 0.5, w, h)
    }

    pub fn track(right: f32, cy: f32) -> Rect {
        Self::frame(right, cy).inset(6.0)
    }

    pub fn paint(&self, p: &mut Painter, right: f32, cy: f32) -> Rect {
        let (_, h) = Self::BOX;
        let frame = Self::frame(right, cy);
        let bg = if self.hovered { p.palette().hover } else { p.palette().well };
        p.quad(Quad::new(frame, 6.0, bg));
        let strip = Self::track(right, cy);
        let hue = ultra_hue(self.time);
        let pattern = if self.ultra {
            Pattern::Ultra { phase: hue - colors::ULTRA_SWEEP }
        } else {
            Pattern::Meter { level: self.level.max(0.08) }
        };
        p.quad(Quad::new(strip, 1.5, p.palette().well).pattern(pattern));
        let color = if self.ultra {
            Color::hsl(hue, colors::ULTRA_SATURATION, p.palette().ultra_lightness)
        } else {
            p.palette().text_secondary
        };
        let layout = p.layout(self.label, typography::METER, None);
        let label_x = frame.x - 10.0 - layout.width;
        p.text(&layout, Point::new(label_x, (cy - layout.height * 0.5).round()), color);
        Rect::new(label_x - 6.0, frame.y, right - label_x + 6.0, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ultra_hue_turns_once_per_cycle() {
        let start = ultra_hue(0.0);
        assert!((ultra_hue(motion::ULTRA_CYCLE) - start).abs() < 1e-4);
        let quarter = ultra_hue(motion::ULTRA_CYCLE * 0.25);
        assert!(((quarter - start).rem_euclid(1.0) - 0.25).abs() < 1e-4);
    }
}
