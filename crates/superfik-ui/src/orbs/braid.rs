//! Braid: three strands plait around the sphere, the `weaving` state.
//! Each strand runs pole to pole on a helix, and a radial breathing term
//! makes them trade places, reading as the over/under of a plait.

use std::f64::consts::PI;

use super::core::{Dot, OrbFrame, Projector, fib_dir, finalize_frame, frac, radius_scale};
use super::profiles::Opts;

pub fn frame(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let (cx, cy) = (size / 2.0, size / 2.0);
    let big_r = (size / 2.0) * 0.76;
    let pt = Projector::new(t * 0.4, 0.3, cx, cy, 1.0);
    let rs = radius_scale(size, o.get("rsPow", 0.6));

    let mut dots = Vec::new();
    let ghost_n = o.get("ghostN", 150.0) as usize;
    for i in 0..ghost_n {
        let d = fib_dir(i, ghost_n);
        let (px, py, z) = pt.project(d[0] * big_r, d[1] * big_r, d[2] * big_r);
        let depth = (z / big_r + 1.0) / 2.0;
        dots.push(Dot { x: px, y: py, z, r: 0.8 * rs, white: 0.78, a: 0.1 + 0.22 * depth });
    }

    let strand_n = o.get("strandN", 52.0) as usize;
    let turns = o.get("turns", 3.0);
    let (r_base, r_depth) = (o.get("rBase", 1.2), o.get("rDepth", 1.8));
    for s in 0..3 {
        let phase = (s as f64 / 3.0) * 2.0 * PI;
        for i in 0..strand_n {
            let u = (frac(i as f64 / strand_n as f64 + t * 0.045) * 2.0 - 1.0) * 0.96;
            let surf = (1.0 - u * u).max(0.0).sqrt();
            let end_fade = ((1.0 - u.abs()) / 0.1).min(1.0);
            let a = u * PI * turns + phase;
            let weave = 1.0 + 0.075 * (u * PI * turns * 2.0 + phase * 2.0 + t * 0.8).sin();
            let rr = surf * big_r * weave;
            let (px, py, zr) = pt.project(a.cos() * rr, u * big_r * weave, a.sin() * rr);
            let depth = (zr / big_r + 1.0) / 2.0;
            dots.push(Dot {
                x: px,
                y: py,
                z: zr,
                r: (r_base + r_depth * depth) * rs,
                white: 0.55 - 0.45 * depth,
                a: end_fade * (0.45 + 0.55 * depth),
            });
        }
    }
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.3))
}
