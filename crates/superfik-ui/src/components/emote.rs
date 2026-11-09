//! Paints one frame of the emote: the light ring, the eyes and a mark,
//! scaled to a width and centred on a point.

use superfik_render::{Color, Point, Quad, Rect};

use crate::emote::{self, EmoteFrame, EmoteState, Eye, Mark, ring_color};
use crate::icons;
use crate::painter::Painter;
use crate::theme::typography;

#[derive(Debug, Clone, Copy)]
pub struct Emote {
    pub state: EmoteState,
    pub from: Option<(EmoteState, f32)>,
    pub time: f32,
    pub width: f32,
}

impl Emote {
    pub fn paint(&self, p: &mut Painter, center: Point) {
        let frame = emote::frame(self.state, self.from, self.time);
        let s = self.width;
        let at = |x: f32, y: f32| Point::new(center.x + x * s, center.y + y * s);
        paint_ring(p, &frame, s, at);
        for eye in &frame.eyes {
            paint_eye(p, eye, s, at);
        }
        for (mark, opacity) in &frame.marks {
            paint_mark(p, *mark, *opacity, s, at);
        }
    }
}

fn dot(p: &mut Painter, center: Point, r: f32, color: Color) {
    p.quad(Quad::new(Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0), r, color));
}

fn paint_ring(p: &mut Painter, frame: &EmoteFrame, s: f32, at: impl Fn(f32, f32) -> Point) {
    let c = p.palette();
    let dim = 1.0 - 0.3 * frame.alert * frame.pulse;
    for d in &frame.ring {
        let color = ring_color(&c.emote_ring, d.stop).mix(c.emote_alert, frame.alert * 0.85).faded(d.a * dim);
        dot(p, at(d.x, d.y), d.r * s, color);
    }
}

fn paint_eye(p: &mut Painter, eye: &Eye, s: f32, at: impl Fn(f32, f32) -> Point) {
    let ink = p.palette().emote_eye;
    let center = at(eye.x, eye.y);
    if eye.smile < 1.0 {
        let (w, h) = (eye.w * s, eye.h * s);
        let rect = Rect::new(center.x - w * 0.5, center.y - h * 0.5, w, h);
        p.quad(Quad::new(rect, w * 0.5, ink.faded(1.0 - eye.smile)));
    }
    if eye.smile > 0.0 {
        let (span, lift, r) = (eye.w * 1.7 * s, eye.h * 0.45 * s, eye.w * 0.36 * s);
        let color = ink.faded(eye.smile);
        for i in 0..=8 {
            let f = i as f32 / 8.0;
            let x = center.x + (f - 0.5) * span;
            let y = center.y + lift * (1.0 - (std::f32::consts::PI * f).sin());
            dot(p, Point::new(x, y), r, color);
        }
    }
}

fn paint_mark(p: &mut Painter, mark: Mark, opacity: f32, s: f32, at: impl Fn(f32, f32) -> Point) {
    let c = p.palette();
    match mark {
        Mark::Exclamation => {
            let color = c.emote_alert.faded(opacity);
            let base = at(0.68, 0.12);
            let (w, h) = (0.085 * s, 0.3 * s);
            p.quad(Quad::new(Rect::new(base.x - w * 0.5, base.y - h, w, h), w * 0.5, color));
            dot(p, Point::new(base.x, base.y + 0.1 * s), w * 0.5, color);
        }
        Mark::Question => {
            let base = at(0.68, 0.0);
            let layout = p.layout("?", typography::EMOTE_MARK, None);
            let origin = Point::new(base.x - layout.width * 0.5, (base.y - layout.height * 0.5).round());
            p.text(&layout, origin, c.emote_alert.faded(opacity));
        }
        Mark::Thumb => {
            p.icon_at(icons::THUMB_UP, at(0.7, 0.02), 0.5 * s, Some(c.emote_done.faded(opacity)));
        }
    }
}
