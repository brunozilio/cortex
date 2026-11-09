//! Alert: the dots assemble a warning sign, a triangle outline with an
//! exclamation mark, pulsing and with a bright wave running round the
//! edge, for a session that needs the user's attention.

use super::core::{Dot, OrbFrame, finalize_frame};
use super::profiles::Opts;

const VERTS: [[f64; 2]; 3] = [[0.0, -0.31], [0.31, 0.23], [-0.31, 0.23]];
const EDGE_DOTS: usize = 21;

pub fn frame(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let c = size / 2.0;
    let dot_mul = o.get("rSizeMul", 1.0);
    let pulse = 1.0 + 0.045 * (t * 4.2).sin();
    let at = |x: f64, y: f64, r: f64, white: f64| Dot {
        x: c + x * pulse * size,
        y: c + (y + 0.02) * pulse * size,
        z: 0.0,
        r: (r * size * dot_mul).max(0.35),
        white,
        a: 1.0,
    };
    let mut dots = Vec::with_capacity(EDGE_DOTS + 4);

    let lengths: Vec<f64> =
        (0..3).map(|i| (VERTS[(i + 1) % 3][0] - VERTS[i][0]).hypot(VERTS[(i + 1) % 3][1] - VERTS[i][1])).collect();
    let total: f64 = lengths.iter().sum();
    for k in 0..EDGE_DOTS {
        let f = k as f64 / EDGE_DOTS as f64;
        let mut target = f * total;
        let mut i = 0;
        while target > lengths[i] && i < 2 {
            target -= lengths[i];
            i += 1;
        }
        let (a, b) = (VERTS[i], VERTS[(i + 1) % 3]);
        let ff = target / lengths[i];
        let wave = 0.5 + 0.5 * (t * 5.0 - f * std::f64::consts::TAU * 2.0).sin();
        dots.push(at(a[0] + (b[0] - a[0]) * ff, a[1] + (b[1] - a[1]) * ff, 0.03, 0.12 + 0.4 * wave));
    }
    for (i, y) in [-0.11, -0.03, 0.05].into_iter().enumerate() {
        dots.push(at(0.0, y, 0.034 - 0.003 * i as f64, 0.05));
    }
    dots.push(at(0.0, 0.145, 0.038, 0.05));
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.25))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_fits_the_square_and_pulses() {
        let a = frame(20.0, 0.0, &Opts::default());
        let b = frame(20.0, 0.37, &Opts::default());
        assert_eq!(a.dots.len(), EDGE_DOTS + 4);
        assert!(a.dots.iter().all(|d| d.x > 0.0 && d.x < 20.0 && d.y > 0.0 && d.y < 20.0));
        assert!(a.dots.iter().zip(&b.dots).any(|(p, q)| (p.x - q.x).abs() > 1e-3));
    }
}
