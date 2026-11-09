//! Face: a small robot made of dots, two eyes and a mouth, for a session
//! with nothing going on. It blinks now and then, glances around on slow
//! noise and bobs very slightly, so it reads as alive rather than parked.
//! Coordinates are fractions of the orb square, like the morph outline.

use super::core::{Dot, OrbFrame, finalize_frame, hash_d, vnoise};
use super::profiles::Opts;

const BLINK_PERIOD: f64 = 3.6;
const BLINK_CLOSE: f64 = 0.09;
const BLINK_OPEN: f64 = 0.14;
const EYE_X: f64 = 0.17;
const EYE_Y: f64 = -0.085;
const EYE_R: f64 = 0.078;
const MOUTH_HALF: f64 = 0.19;
const MOUTH_Y: f64 = 0.15;
const MOUTH_DOTS: usize = 7;

fn blink(t: f64) -> f64 {
    let cycle = (t / BLINK_PERIOD).floor();
    let at = cycle * BLINK_PERIOD + 2.4 + hash_d(cycle, 3.3) * 1.0;
    let dt = t - at;
    if dt < 0.0 {
        0.0
    } else if dt < BLINK_CLOSE {
        dt / BLINK_CLOSE
    } else if dt < BLINK_CLOSE + BLINK_OPEN {
        1.0 - (dt - BLINK_CLOSE) / BLINK_OPEN
    } else {
        0.0
    }
}

pub fn frame(size: f64, t: f64, o: &Opts) -> OrbFrame {
    let c = size / 2.0;
    let dot_mul = o.get("rSizeMul", 1.0);
    let at = |x: f64, y: f64, r: f64, white: f64, a: f64| Dot {
        x: c + x * size,
        y: c + y * size,
        z: 0.0,
        r: (r * size * dot_mul).max(0.35),
        white,
        a,
    };
    let mut dots = Vec::with_capacity(4 + 6 + MOUTH_DOTS);

    let gx = (vnoise(t * 0.33, 1.5) - 0.5) * 0.07;
    let gy = (vnoise(t * 0.27, 7.5) - 0.5) * 0.04;
    let bob = 0.008 * (t * 1.1).sin();
    let shut = blink(t);

    for side in [-1.0, 1.0] {
        let (ex, ey) = (side * EYE_X + gx, EYE_Y + gy + bob);
        let r = EYE_R * (1.0 - 0.55 * shut);
        dots.push(at(ex, ey, r, 0.08, 1.0));
        for lid in [-1.0, 1.0] {
            let lx = ex + lid * EYE_R * 0.95 * shut;
            dots.push(at(lx, ey, EYE_R * 0.45 * shut, 0.08, shut));
        }
    }

    let smile = 0.05 + 0.025 * (t * 0.7).sin();
    for i in 0..MOUTH_DOTS {
        let u = i as f64 / (MOUTH_DOTS - 1) as f64 * 2.0 - 1.0;
        let x = u * MOUTH_HALF + gx * 0.5;
        let y = MOUTH_Y - smile * 0.6 + gy * 0.5 + bob + smile * (1.0 - u * u);
        let end = 1.0 - 0.25 * u.abs();
        dots.push(at(x, y, 0.036 * end, 0.1 + 0.12 * u.abs(), 1.0));
    }
    dots.push(at(gx * 0.3, -0.33 + bob, 0.03, 0.3, 1.0));
    dots.push(at(gx * 0.15, -0.26 + bob, 0.022, 0.4, 1.0));
    finalize_frame(dots, Vec::new(), o.get("rMin", 0.25))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eyes_blink_once_per_cycle() {
        let shut: Vec<f64> = (0..360).map(|i| blink(i as f64 / 100.0)).collect();
        let closed = shut.iter().filter(|s| **s > 0.9).count();
        assert!((1..=6).contains(&closed), "{closed}");
        assert_eq!(blink(0.0), 0.0);
    }

    #[test]
    fn face_has_eyes_and_mouth_inside_the_square() {
        let f = frame(20.0, 1.0, &Opts::default());
        assert!(f.dots.len() >= 2 + MOUTH_DOTS);
        assert!(f.dots.iter().all(|d| d.x > 0.0 && d.x < 20.0 && d.y > 0.0 && d.y < 20.0));
    }
}
