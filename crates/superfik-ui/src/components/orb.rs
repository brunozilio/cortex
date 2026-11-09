//! The thought-orb: paints one frame of a dotted orb state, far dots
//! first, morphing from the previous state when one is given.

use superfik_motion::Easing;
use superfik_render::{Color, Point, Quad, Rect};

use crate::orbs::{self, OrbFrame, OrbSize, OrbState};
use crate::painter::Painter;
use crate::theme::colors::Palette;

#[derive(Debug, Clone, Copy)]
pub struct Orb {
    pub state: OrbState,
    pub size: OrbSize,
    pub time: f64,
    pub from: Option<(OrbState, f32)>,
}

impl Orb {
    pub fn paint(&self, p: &mut Painter, origin: Point) {
        let target = orbs::frame(self.state, self.size, self.time);
        let frame = match self.from.filter(|(_, k)| *k < 1.0) {
            Some((previous, k)) => {
                let from = orbs::frame(previous, self.size, self.time);
                orbs::blend(&from, &target, self.size.px(), f64::from(Easing::IN_OUT.apply(k)))
            }
            None => target,
        };
        paint_frame(p, &frame, origin);
    }
}

fn ink(c: &Palette, white: f64, alpha: f64) -> Color {
    let near = (1.0 - white.clamp(0.0, 1.0)) as f32;
    c.orb_ink_far.mix(c.orb_ink_near, near).faded(alpha as f32)
}

fn dot(p: &mut Painter, x: f64, y: f64, r: f64, color: Color) {
    let (x, y, r) = (x as f32, y as f32, r as f32);
    p.quad(Quad::new(Rect::new(x - r, y - r, r * 2.0, r * 2.0), r, color));
}

pub fn paint_frame(p: &mut Painter, frame: &OrbFrame, origin: Point) {
    let (ox, oy) = (f64::from(origin.x), f64::from(origin.y));
    for line in &frame.lines {
        let color = ink(p.palette(), line.white, line.a);
        let (dx, dy) = (line.x2 - line.x1, line.y2 - line.y1);
        let length = (dx * dx + dy * dy).sqrt();
        let step = (line.w * 0.75).max(0.5);
        let steps = ((length / step).ceil() as usize).max(1);
        for i in 0..=steps {
            let f = i as f64 / steps as f64;
            dot(p, ox + line.x1 + dx * f, oy + line.y1 + dy * f, line.w * 0.5, color);
        }
    }
    for d in &frame.dots {
        dot(p, ox + d.x, oy + d.y, d.r, ink(p.palette(), d.white, d.a));
    }
}
