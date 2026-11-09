//! The browser toolbar's drawn glyphs: the expand arrows, which turn
//! inward as the card grows over the window, and the loading spinner the
//! reload button becomes. Strokes are wells (`GooShape::Capsule` / `Arc`)
//! that do not melt into each other.

use std::f32::consts::{FRAC_1_SQRT_2, PI, TAU};

use superfik_render::{Color, GooShape, Point};

use crate::painter::Painter;
use crate::theme::{metrics, motion};

const STROKE: f32 = metrics::GLYPH_STROKE * 0.5;

pub fn expand_arrows(p: &mut Painter, center: Point, size: f32, turn: f32, color: Color) {
    let half = size * 0.5;
    let (inner, outer, head) = (half * 0.18, half * 0.9, half * 0.42);
    let angle = PI * turn.clamp(0.0, 1.0);
    for side in [1.0_f32, -1.0] {
        let along = Point::new(FRAC_1_SQRT_2 * side, -FRAC_1_SQRT_2 * side);
        let at = |d: f32| Point::new(along.x * d, along.y * d);
        let middle = at((inner + outer) * 0.5);
        let tip = at(outer);
        let arms = [Point::new(tip.x - along.x * head, tip.y), Point::new(tip.x, tip.y - along.y * head)];
        let turned = |q: Point| {
            let (dx, dy) = (q.x - middle.x, q.y - middle.y);
            let (sin, cos) = angle.sin_cos();
            Point::new(center.x + middle.x + dx * cos - dy * sin, center.y + middle.y + dx * sin + dy * cos)
        };
        stroke(p, turned(at(inner)), turned(tip), color);
        for arm in arms {
            stroke(p, turned(tip), turned(arm), color);
        }
    }
}

pub(crate) fn spinner(p: &mut Painter, center: Point, size: f32, time: f32, color: Color) {
    let radius = size * 0.5 - STROKE;
    let start = (time * TAU * motion::SPINNER_TURNS) % TAU;
    let shape = GooShape::Arc { center, radius, start, sweep: TAU * 0.72, thickness: STROKE };
    p.well(shape, color, 0.0);
}

fn stroke(p: &mut Painter, a: Point, b: Point, color: Color) {
    p.well(GooShape::Capsule { a, b, radius: STROKE }, color, 0.0);
}
