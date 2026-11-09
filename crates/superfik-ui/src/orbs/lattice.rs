//! The sphere-lattice modes: globe (`searching`), rubik (`solving`) and
//! wave (`listening`). All draw a lat/long dot field with mode-specific
//! motion, then hand off to the shared z-sorted finisher.

use std::f64::consts::PI;

use super::core::{Dot, OrbFrame, Projector, angle_delta, finalize_frame, hash_d, radius_scale};
use super::profiles::Opts;

fn lattice(rings: usize, lon_density: f64, mut f: impl FnMut(f64, f64, usize)) {
    for li in 0..=rings {
        let lat = -PI / 2.0 + (li as f64 / rings as f64) * PI;
        let lon_count = ((lat.cos().abs() * lon_density).round() as usize).max(1);
        for lj in 0..lon_count {
            let lon = (lj as f64 / lon_count as f64) * 2.0 * PI;
            f(lat, lon, li);
        }
    }
}

pub fn globe(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let spin = 0.5;
    let (cx, cy) = (size / 2.0, size / 2.0);
    let radius = (size / 2.0) * 0.82;
    let tilt = 0.4 + 0.06 * (t * 0.35).sin();
    let pt = Projector::new(t * spin, tilt, cx, cy, radius);
    let scan = t * (spin + (1.7 - spin) * o.get("scanMul", 1.0));
    let rs = radius_scale(size, o.get("rsPow", 0.6));
    let dim_base = o.get("dimBase", 1.0);
    let (r_base, r_depth, r_boost) = (o.get("rBase", 0.6), o.get("rDepth", 1.7), o.get("rBoost", 1.0));
    let (ink_far, ink_span) = (o.get("inkFar", 0.62), o.get("inkSpan", 0.54));

    let mut dots = Vec::new();
    lattice(o.get("latRings", 17.0) as usize, o.get("lonDensity", 44.0), |lat, lon, _| {
        let (px, py, z) = pt.project(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
        let depth = (z + 1.0) / 2.0;
        let d = angle_delta(lon + t * spin, scan);
        let boost = (-(d * d) / 0.18).exp() * z.max(0.0);
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: (r_base + r_depth * depth + r_boost * boost) * rs,
            white: ink_far - ink_span * depth,
            a: dim_base + (1.0 - dim_base) * boost.min(1.0),
        });
    });
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.3))
}

struct Move {
    axis: u8,
    lo: f64,
    hi: f64,
    ang: f64,
}

fn solve_cycle(time: f64, count: usize, slot_dur: f64, rest: f64) -> (Vec<f64>, Option<usize>) {
    let cyc = 2.0 * count as f64 * slot_dur + rest;
    let tc = time % cyc;
    let mut amount = vec![0.0; count];
    let mut active = None;
    if tc < 2.0 * count as f64 * slot_dur {
        let slot = (tc / slot_dur).floor() as usize;
        let p = (tc - slot as f64 * slot_dur) / slot_dur;
        let cl = (p / 0.7).min(1.0);
        let ep = 1.0 - (1.0 - cl).powi(3);
        if slot < count {
            for a in amount.iter_mut().take(slot) {
                *a = 1.0;
            }
            amount[slot] = ep;
            active = Some(slot);
        } else {
            let u = 2 * count - 1 - slot;
            for a in amount.iter_mut().take(u) {
                *a = 1.0;
            }
            amount[u] = 1.0 - ep;
            active = Some(u);
        }
    }
    (amount, active)
}

fn apply_moves(mut p: [f64; 3], moves: &[Move], amount: &[f64], active: Option<usize>) -> ([f64; 3], bool) {
    let mut in_active = false;
    for (i, mv) in moves.iter().enumerate() {
        if amount[i] <= 0.0 {
            continue;
        }
        let coord = p[mv.axis as usize];
        if coord < mv.lo || coord >= mv.hi {
            continue;
        }
        if Some(i) == active {
            in_active = true;
        }
        let a = mv.ang * amount[i];
        let (ca, sa) = (a.cos(), a.sin());
        let [x, y, z] = p;
        p = match mv.axis {
            0 => [x, y * ca - z * sa, y * sa + z * ca],
            1 => [x * ca + z * sa, y, -x * sa + z * ca],
            _ => [x * ca - y * sa, x * sa + y * ca, z],
        };
    }
    (p, in_active)
}

fn make_moves(count: usize) -> Vec<Move> {
    (0..count)
        .map(|i| {
            let i = i as f64;
            let axis = ((hash_d(i, 2.3) * 3.0).floor() as u8).min(2);
            let lo = -1.0 + 0.5 * (hash_d(i, 5.9) * 4.0).floor().min(3.0);
            let dir = if hash_d(i, 7.7) < 0.5 { 1.0 } else { -1.0 };
            Move { axis, lo, hi: lo + 0.5, ang: dir * PI / 2.0 }
        })
        .collect()
}

pub fn rubik(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let (cx, cy) = (size / 2.0, size / 2.0);
    let big_r = (size / 2.0) * 0.82;
    let pt = Projector::new(t * 0.55, 0.35 + 0.1 * (t * 0.9).sin(), cx, cy, big_r);
    let rs = radius_scale(size, o.get("rsPow", 0.6));
    let move_count = o.get("moveCount", 14.0) as usize;
    let moves = make_moves(move_count);
    let (amount, active) = solve_cycle(t, move_count, 0.42, 1.2);
    let (r_base, r_depth, r_active) = (o.get("rBase", 0.6), o.get("rDepth", 1.7), o.get("rActive", 0.3));
    let (ink_far, ink_span) = (o.get("inkFar", 0.62), o.get("inkSpan", 0.54));

    let mut dots = Vec::new();
    lattice(o.get("latRings", 15.0) as usize, o.get("lonDensity", 40.0), |lat, lon, _| {
        let (p, in_active) =
            apply_moves([lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin()], &moves, &amount, active);
        let (px, py, zr) = pt.project(p[0], p[1], p[2]);
        let depth = (zr + 1.0) / 2.0;
        dots.push(Dot {
            x: px,
            y: py,
            z: zr,
            r: (r_base + r_depth * depth + if in_active { r_active } else { 0.0 }) * rs,
            white: ink_far - ink_span * depth - if in_active { 0.14 } else { 0.0 },
            a: 1.0,
        });
    });
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.3))
}

pub fn wave(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let (cx, cy) = (size / 2.0, size / 2.0);
    let big_r = (size / 2.0) * 0.874;
    let pt = Projector::new(t * 0.18, 0.38, cx, cy, 1.0);
    let rs = radius_scale(size, o.get("rsPow", 0.6));
    let (r_base, r_depth) = (o.get("rBase", 0.6), o.get("rDepth", 1.7));

    let mut dots = Vec::new();
    lattice(o.get("rings", 15.0) as usize, o.get("lonDensity", 40.0), |lat, lon, ri| {
        let ri = ri as f64;
        let w = 0.62 * (t * 2.1 - ri * 0.52).sin() + 0.38 * (t * 1.27 + ri * 0.83).sin();
        let rr = big_r * (0.88 + 0.105 * w);
        let (px, py, z) = pt.project(lat.cos() * lon.cos() * rr, lat.sin() * rr, lat.cos() * lon.sin() * rr);
        let depth = (z / big_r + 1.0) / 2.0;
        let crest = w.max(0.0);
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: (r_base + r_depth * depth) * (1.0 + 0.4 * crest) * rs,
            white: 0.66 - 0.56 * depth - 0.1 * crest,
            a: 1.0,
        });
    });
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.3))
}
