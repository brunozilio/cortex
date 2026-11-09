//! A CPU rasteriser for the emote, for places the GPU scene cannot reach
//! (the Dock tile). Anti-aliased dots and rounded boxes composited over a
//! transparent canvas; the marks come from the icon set.

use superfik_render::{Color, icons};

use super::{EmoteFrame, Eye, Mark, ring_color};
use crate::icons as glyphs;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub ring: [Color; 12],
    pub eye: Color,
    pub alert: Color,
    pub done: Color,
    pub badge: Color,
    pub tile_top: Color,
    pub tile_bottom: Color,
    pub tile_rim: Color,
}

const TILE_SPAN: f32 = 0.805;
const TILE_RADIUS: f32 = 0.2237;
const RING_SPAN: f32 = 0.6;
const BADGE: f32 = 0.2;

struct Canvas {
    size: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn blend(&mut self, x: usize, y: usize, color: Color, coverage: f32) {
        let a = color.a * coverage;
        if a <= 0.0 {
            return;
        }
        let dst = &mut self.px[y * self.size + x];
        let keep = 1.0 - a;
        dst[0] = color.r * a + dst[0] * keep;
        dst[1] = color.g * a + dst[1] * keep;
        dst[2] = color.b * a + dst[2] * keep;
        dst[3] = a + dst[3] * keep;
    }

    fn bbox(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Option<(usize, usize, usize, usize)> {
        let last = (self.size - 1) as f32;
        let (ax, ay) = (x0.floor().clamp(0.0, last) as usize, y0.floor().clamp(0.0, last) as usize);
        let (bx, by) = (x1.ceil().clamp(0.0, last) as usize, y1.ceil().clamp(0.0, last) as usize);
        (ax <= bx && ay <= by).then_some((ax, ay, bx, by))
    }

    fn circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let Some((ax, ay, bx, by)) = self.bbox(cx - r - 1.0, cy - r - 1.0, cx + r + 1.0, cy + r + 1.0) else { return };
        for y in ay..=by {
            for x in ax..=bx {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                self.blend(x, y, color, (r + 0.5 - d).clamp(0.0, 1.0));
            }
        }
    }

    fn rounded_box(&mut self, cx: f32, cy: f32, w: f32, h: f32, radius: f32, color: Color) {
        self.rounded_box_sheen(cx, cy, w, h, radius, color, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn rounded_box_sheen(&mut self, cx: f32, cy: f32, w: f32, h: f32, radius: f32, top: Color, bottom: Color) {
        let (hw, hh) = (w * 0.5, h * 0.5);
        let r = radius.min(hw).min(hh);
        let Some((ax, ay, bx, by)) = self.bbox(cx - hw - 1.0, cy - hh - 1.0, cx + hw + 1.0, cy + hh + 1.0) else {
            return;
        };
        for y in ay..=by {
            let color = top.mix(bottom, ((y as f32 + 0.5 - cy + hh) / h).clamp(0.0, 1.0));
            for x in ax..=bx {
                let (qx, qy) = ((x as f32 + 0.5 - cx).abs() - hw + r, (y as f32 + 0.5 - cy).abs() - hh + r);
                let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
                let d = outside + qx.max(qy).min(0.0) - r;
                self.blend(x, y, color, (0.5 - d).clamp(0.0, 1.0));
            }
        }
    }

    fn icon(&mut self, icon: &icons::Icon, cx: f32, cy: f32, size: f32, color: Color) {
        let px = size.round().max(1.0) as u32;
        let Some(bitmap) = icons::rasterize(icon, px, px) else { return };
        let (ox, oy) = (cx - size * 0.5, cy - size * 0.5);
        for y in 0..px as usize {
            for x in 0..px as usize {
                let coverage = f32::from(bitmap[(y * px as usize + x) * 4 + 3]) / 255.0;
                let (tx, ty) = (ox + x as f32, oy + y as f32);
                if tx >= 0.0 && ty >= 0.0 && (tx as usize) < self.size && (ty as usize) < self.size {
                    self.blend(tx as usize, ty as usize, color, coverage);
                }
            }
        }
    }

    fn into_rgba(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for [r, g, b, a] in self.px {
            let un = |c: f32| if a > 0.0 { (c / a * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
            out.extend_from_slice(&[un(r), un(g), un(b), (a * 255.0).round().clamp(0.0, 255.0) as u8]);
        }
        out
    }
}

pub fn raster(frame: &EmoteFrame, look: &Look, size: u32) -> Vec<u8> {
    let size = size.max(1) as usize;
    let mut canvas = Canvas { size, px: vec![[0.0; 4]; size * size] };
    let s = size as f32 * RING_SPAN;
    let (cx, cy) = (size as f32 * 0.5, size as f32 * 0.5);
    let at = |x: f32, y: f32| (cx + x * s, cy + y * s);
    let tile = size as f32 * TILE_SPAN;
    let rim = (size as f32 / 128.0).max(1.0);
    canvas.rounded_box(cx, cy, tile, tile, tile * TILE_RADIUS, look.tile_rim);
    canvas.rounded_box_sheen(
        cx,
        cy,
        tile - 2.0 * rim,
        tile - 2.0 * rim,
        tile * TILE_RADIUS - rim,
        look.tile_top,
        look.tile_bottom,
    );
    let dim = 1.0 - 0.3 * frame.alert * frame.pulse;
    for d in &frame.ring {
        let color = ring_color(&look.ring, d.stop).mix(look.alert, frame.alert * 0.85).faded(d.a * dim);
        let (x, y) = at(d.x, d.y);
        canvas.circle(x, y, d.r * s, color);
    }
    for eye in &frame.eyes {
        paint_eye(&mut canvas, eye, look.eye, s, at);
    }
    for (mark, opacity) in &frame.marks {
        let (x, y) = match mark {
            Mark::Thumb => at(0.44, 0.38),
            Mark::Exclamation | Mark::Question => at(0.44, -0.38),
        };
        canvas.circle(x, y, BADGE * s, look.badge.faded(*opacity));
        match mark {
            Mark::Exclamation => {
                let color = look.alert.faded(*opacity);
                canvas.rounded_box(x, y - 0.045 * s, 0.075 * s, 0.22 * s, 0.04 * s, color);
                canvas.circle(x, y + 0.13 * s, 0.04 * s, color);
            }
            Mark::Question => canvas.icon(&glyphs::QUESTION, x, y, 0.36 * s, look.alert.faded(*opacity)),
            Mark::Thumb => canvas.icon(&glyphs::THUMB_UP, x, y, 0.34 * s, look.done.faded(*opacity)),
        }
    }
    canvas.into_rgba()
}

fn paint_eye(canvas: &mut Canvas, eye: &Eye, ink: Color, s: f32, at: impl Fn(f32, f32) -> (f32, f32)) {
    let (x, y) = at(eye.x, eye.y);
    if eye.smile < 1.0 {
        let (w, h) = (eye.w * s, eye.h * s);
        canvas.rounded_box(x, y, w, h, w * 0.5, ink.faded(1.0 - eye.smile));
    }
    if eye.smile > 0.0 {
        let (span, lift, r) = (eye.w * 1.7 * s, eye.h * 0.45 * s, eye.w * 0.36 * s);
        let color = ink.faded(eye.smile);
        for i in 0..=8 {
            let f = i as f32 / 8.0;
            let dy = lift * (1.0 - (std::f32::consts::PI * f).sin());
            canvas.circle(x + (f - 0.5) * span, y + dy, r, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emote::{EmoteState, frame};
    use crate::theme::colors::ICON_LOOK;

    fn alpha_at(rgba: &[u8], size: usize, x: usize, y: usize) -> u8 {
        rgba[(y * size + x) * 4 + 3]
    }

    #[test]
    fn paints_a_ring_with_a_clear_centre_and_white_eyes() {
        let size = 64;
        let rgba = raster(&frame(EmoteState::Idle, None, 0.0), &ICON_LOOK, size as u32);
        assert_eq!(rgba.len(), size * size * 4);
        assert_eq!(alpha_at(&rgba, size, 1, 1), 0, "corners stay transparent");
        assert!(alpha_at(&rgba, size, 32, 8) > 200, "the tile is opaque");
        let i = (20 * size + 32) * 4;
        assert!(rgba[i] < 60 && rgba[i + 1] < 60 && rgba[i + 2] < 60, "inside the ring is the dark tile");
        let i = (14 * size + 32) * 4;
        assert!(rgba[i].max(rgba[i + 1]).max(rgba[i + 2]) > 150, "the ring is lit");
        let eye = (31 - 5, 32);
        let i = (eye.1 * size + eye.0) * 4;
        assert!(rgba[i] > 240 && rgba[i + 1] > 240 && rgba[i + 2] > 240, "the eye is white");
    }

    #[test]
    fn a_finished_run_wears_a_green_thumb_on_the_bottom_right() {
        let size = 128;
        let greens = |rgba: &[u8]| {
            (92..118)
                .flat_map(|y| (92..118).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let i = (y * size + x) * 4;
                    let (r, g, b) = (u16::from(rgba[i]), u16::from(rgba[i + 1]), u16::from(rgba[i + 2]));
                    rgba[i + 3] > 0 && g > r + 40 && g > b + 40
                })
                .count()
        };
        assert_eq!(greens(&raster(&frame(EmoteState::Idle, None, 0.0), &ICON_LOOK, size as u32)), 0);
        assert!(greens(&raster(&frame(EmoteState::Done, None, 0.0), &ICON_LOOK, size as u32)) > 10);
    }
}
