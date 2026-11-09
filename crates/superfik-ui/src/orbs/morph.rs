//! Morph: a dotted outline cycling circle → triangle → square → circle,
//! the `shaping` state. Each shape is a closed path parameterised by arc
//! length (top-centre start, clockwise). Every frame blends the two
//! neighbouring paths, then lays the dots evenly along the blended
//! outline, so spacing stays uniform through holds and transitions alike.

use std::f64::consts::PI;

use super::core::{Dot, OrbFrame, finalize_frame};
use super::profiles::Opts;

enum Path {
    Circle,
    Poly { verts: Vec<[f64; 2]>, lengths: Vec<f64>, total: f64 },
}

impl Path {
    fn poly(verts: &[[f64; 2]]) -> Self {
        let n = verts.len();
        let lengths: Vec<f64> =
            (0..n).map(|i| (verts[(i + 1) % n][0] - verts[i][0]).hypot(verts[(i + 1) % n][1] - verts[i][1])).collect();
        let total = lengths.iter().sum();
        Self::Poly { verts: verts.to_vec(), lengths, total }
    }

    fn at(&self, f: f64) -> [f64; 2] {
        match self {
            Self::Circle => {
                let a = -PI / 2.0 + f * 2.0 * PI;
                [a.cos() * 0.24, a.sin() * 0.24]
            }
            Self::Poly { verts, lengths, total } => {
                let n = verts.len();
                let mut target = f * total;
                let mut i = 0;
                while target > lengths[i] && i < n - 1 {
                    target -= lengths[i];
                    i += 1;
                }
                let a = verts[i];
                let b = verts[(i + 1) % n];
                let ff = if lengths[i] != 0.0 { (target / lengths[i]).min(1.0) } else { 0.0 };
                [a[0] + (b[0] - a[0]) * ff, a[1] + (b[1] - a[1]) * ff]
            }
        }
    }
}

fn cycle() -> [Path; 3] {
    [
        Path::Circle,
        Path::poly(&[[0.0, -0.26], [0.24, 0.16], [-0.24, 0.16]]),
        Path::poly(&[[0.0, -0.2], [0.2, -0.2], [0.2, 0.2], [-0.2, 0.2], [-0.2, -0.2]]),
    ]
}

fn smooth(x: f64) -> f64 {
    x * x * (3.0 - 2.0 * x)
}

fn morph_n(d: f64) -> usize {
    ((34.0 * d).round() as usize).max(6)
}

const HOLD: f64 = 1.4;
const MORPH: f64 = 0.9;
const SEG: f64 = HOLD + MORPH;

pub fn frame(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let paths = cycle();
    let k_n = paths.len();
    let tc = t % (SEG * k_n as f64);
    let held = o.try_get("shape").filter(|s| *s >= 0.0 && *s < k_n as f64).map(|s| s.floor() as usize);
    let k = held.unwrap_or_else(|| (tc / SEG).floor() as usize);
    let local = if held.is_some() { t % SEG } else { tc - k as f64 * SEG };
    let m = if held.is_some() || local <= HOLD { 0.0 } else { smooth((local - HOLD) / MORPH) };
    let sprd = o.get("spread", 1.0);

    let p_a = &paths[k];
    let p_b = if held.is_some() { p_a } else { &paths[(k + 1) % k_n] };
    const M: usize = 160;
    let pts: Vec<[f64; 2]> = (0..M)
        .map(|i| {
            let f = i as f64 / M as f64;
            let a = p_a.at(f);
            let b = p_b.at(f);
            [(a[0] + (b[0] - a[0]) * m) * sprd, (a[1] + (b[1] - a[1]) * m) * sprd]
        })
        .collect();
    let lengths: Vec<f64> =
        (0..M).map(|i| (pts[(i + 1) % M][0] - pts[i][0]).hypot(pts[(i + 1) % M][1] - pts[i][1])).collect();
    let total: f64 = lengths.iter().sum();

    let n = morph_n(o.get("iconD", 1.0));
    let re = o.get("rDot", 0.021) * 1.35 * sprd;
    let pulse = 1.0 + 0.02 * (local * 3.1).sin();

    let mut dots = Vec::with_capacity(n);
    let c2 = size / 2.0;
    let mut seg = 0;
    let mut acc = 0.0;
    for k2 in 0..n {
        let target = (k2 as f64 / n as f64) * total;
        while acc + lengths[seg] < target && seg < M - 1 {
            acc += lengths[seg];
            seg += 1;
        }
        let a = pts[seg];
        let b = pts[(seg + 1) % M];
        let f = if lengths[seg] != 0.0 { ((target - acc) / lengths[seg]).min(1.0) } else { 0.0 };
        let x = (a[0] + (b[0] - a[0]) * f) * pulse;
        let y = (a[1] + (b[1] - a[1]) * f) * pulse;
        dots.push(Dot { x: c2 + x * size, y: c2 + y * size, z: 0.0, r: (re * size).max(0.35), white: 0.1, a: 1.0 });
    }
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.3))
}
