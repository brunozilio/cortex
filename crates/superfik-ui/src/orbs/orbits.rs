//! Orbits: particles on tilted orbits, the `working` state. No nucleus:
//! just ghost paths and the particles doing the work.

use std::f64::consts::PI;

use super::core::{Dot, OrbFrame, Projector, finalize_frame, hash_d, radius_scale};
use super::profiles::Opts;

pub fn frame(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let (cx, cy) = (size / 2.0, size / 2.0);
    let big_r = (size / 2.0) * 0.82;
    let pt = Projector::new(t * 0.12, 0.3, cx, cy, 1.0);
    let rs = radius_scale(size, o.get("rsPow", 0.6));

    let mut dots = Vec::new();
    let orbit_n = o.get("orbitN", 12.0) as usize;
    let ghost_n = o.get("ghostN", 40.0) as usize;
    let particles = o.get("particles", 3.0) as usize;
    let (ghost_r, ghost_a) = (o.get("ghostR", 0.9), o.get("ghostA", 0.5));
    let (part_r, part_r_depth) = (o.get("partR", 1.2), o.get("partRDepth", 1.6));

    for orb in 0..orbit_n {
        let h1 = hash_d(orb as f64, 1.7);
        let h2 = hash_d(orb as f64, 5.2);
        let h3 = hash_d(orb as f64, 8.9);
        let ro = big_r * (0.45 + 0.52 * h1);
        let th = h1 * 2.0 * PI;
        let phi = (2.0 * h2 - 1.0).acos();
        let nx = phi.sin() * th.cos();
        let ny = phi.cos();
        let nz = phi.sin() * th.sin();
        let mut ux = -ny;
        let mut uy = nx;
        let uz = 0.0;
        let ul = (ux * ux + uy * uy).sqrt().max(1e-6);
        ux /= ul;
        uy /= ul;
        let vx = ny * uz - nz * uy;
        let vy = nz * ux - nx * uz;
        let vz = nx * uy - ny * ux;
        let speed = (0.25 + 0.55 * h3) * if h3 > 0.5 { 1.0 } else { -1.0 };
        let on_orbit = |a: f64| {
            pt.project(
                (ux * a.cos() + vx * a.sin()) * ro,
                (uy * a.cos() + vy * a.sin()) * ro,
                (uz * a.cos() + vz * a.sin()) * ro,
            )
        };

        for k in 0..ghost_n {
            let a = (k as f64 / ghost_n as f64) * 2.0 * PI;
            let (px, py, z) = on_orbit(a);
            let depth = (z / ro + 1.0) / 2.0;
            dots.push(Dot { x: px, y: py, z, r: ghost_r * rs, white: 0.72, a: ghost_a * (0.4 + 0.6 * depth) });
        }
        for m in 0..particles {
            let a = t * speed + (m as f64 / particles as f64) * 2.0 * PI + h2 * 6.0;
            let (px, py, z) = on_orbit(a);
            let depth = (z / ro + 1.0) / 2.0;
            dots.push(Dot {
                x: px,
                y: py,
                z,
                r: (part_r + part_r_depth * depth) * rs,
                white: 0.3 - 0.22 * depth,
                a: 1.0,
            });
        }
    }
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.3))
}
