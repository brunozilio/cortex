//! Ribbon: an undulating sash of parallel strands rides a great circle,
//! the `composing` state. The tuned preset freezes the 3D tumble (spin 0),
//! leaving the travelling undulation on a fixed band.
//!
//! The same geometry drives `breathing` (ring) via the `faceOn` flag: a
//! face-on circle whose radius, not its out-of-plane offset, undulates,
//! so it reads as a ring slowly morphing rather than a sash in orbit.

use std::f64::consts::PI;

use super::core::{Dot, OrbFrame, Projector, fib_dir, finalize_frame, radius_scale};
use super::profiles::Opts;

pub fn frame(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let (cx, cy) = (size / 2.0, size / 2.0);
    let big_r = (size / 2.0) * 0.78;
    let spin = o.get("spin", 1.0);
    let cam_tilt = 0.3;
    let pt = Projector::new(t * 0.1 * spin, cam_tilt, cx, cy, 1.0);
    let rs = radius_scale(size, o.get("rsPow", 0.6));
    let face_on = o.get("faceOn", 0.0) != 0.0;

    let mut dots = Vec::new();
    let ghost_n = o.get("ghostN", 150.0) as usize;
    for i in 0..ghost_n {
        let d = fib_dir(i, ghost_n);
        let (px, py, z) = pt.project(d[0] * big_r, d[1] * big_r, d[2] * big_r);
        let depth = (z / big_r + 1.0) / 2.0;
        dots.push(Dot { x: px, y: py, z, r: 0.8 * rs, white: 0.78, a: 0.1 + 0.22 * depth });
    }

    let ya = t * 0.24 * spin;
    let ta = if face_on { -cam_tilt } else { 0.55 + 0.3 * (t * 0.18).sin() * spin };
    let (ux, uy, uz) = (ya.cos(), 0.0, ya.sin());
    let (vx, vy, vz) = (-uz * ta.sin(), ta.cos(), ux * ta.sin());
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;

    let wob_mul = o.get("wobMul", 1.0);
    let wob_amp = 0.23 * wob_mul;
    let base_r = if face_on { big_r / (1.0 + 0.85 * wob_amp) } else { big_r };

    let base_lanes = o.get("lanes", 5.0);
    let segs = o.get("segs", 88.0) as usize;
    let lanes = ((base_lanes * o.get("bandMul", 1.0)).round() as usize).max(1);
    let (r_base, r_depth) = (o.get("rBase", 1.1), o.get("rDepth", 1.7));
    let half = (lanes as f64 - 1.0) / 2.0;
    for w in 0..lanes {
        let w = w as f64;
        let lane_off = (w - half) * 0.075;
        let edge = (w - half).abs() / half.max(1.0);
        for k in 0..segs {
            let a = (k as f64 / segs as f64) * 2.0 * PI;
            let wob = (0.16 * (a * 3.0 - t * 1.7 + w * 0.22).sin() + 0.07 * (a * 5.0 + t * 1.1).sin()) * wob_mul;
            let radial = if face_on { 1.0 + wob } else { 1.0 };
            let off = if face_on { lane_off } else { lane_off + wob };
            let x = ux * a.cos() + vx * a.sin() + nx * off;
            let y = uy * a.cos() + vy * a.sin() + ny * off;
            let z = uz * a.cos() + vz * a.sin() + nz * off;
            let l = (x * x + y * y + z * z).sqrt();
            let rr = base_r * radial;
            let (px, py, zr) = pt.project((x / l) * rr, (y / l) * rr, (z / l) * rr);
            let depth = (zr / big_r + 1.0) / 2.0;
            dots.push(Dot {
                x: px,
                y: py,
                z: zr,
                r: (r_base + r_depth * depth) * (1.0 - 0.25 * edge) * rs,
                white: 0.52 - 0.44 * depth + 0.18 * edge,
                a: 0.4 + 0.6 * depth,
            });
        }
    }
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.3))
}
