//! Shared primitives: deterministic hashes and noise, the Fibonacci
//! lattice, the spin + tilt orthographic projector, and frame finishing
//! (visibility, radius floor, far→near z-sort).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub r: f64,
    pub white: f64,
    pub a: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub white: f64,
    pub a: f64,
    pub w: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OrbFrame {
    pub dots: Vec<Dot>,
    pub lines: Vec<Line>,
}

pub fn lerp(a: f64, b: f64, f: f64) -> f64 {
    a + (b - a) * f
}

pub fn frac(x: f64) -> f64 {
    x - x.floor()
}

pub fn hash_d(a: f64, b: f64) -> f64 {
    let h = (a * 12.9898 + b * 78.233).sin() * 43758.5453;
    h - h.floor()
}

pub fn vnoise(x: f64, y: f64) -> f64 {
    let xi = x.floor();
    let yi = y.floor();
    let mut fx = x - xi;
    let mut fy = y - yi;
    fx = fx * fx * (3.0 - 2.0 * fx);
    fy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash_d(xi, yi);
    let b = hash_d(xi + 1.0, yi);
    let c = hash_d(xi, yi + 1.0);
    let d = hash_d(xi + 1.0, yi + 1.0);
    a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy
}

pub fn fib_dir(i: usize, n: usize) -> [f64; 3] {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let y = 1.0 - (2.0 * (i as f64 + 0.5)) / n as f64;
    let rad = (1.0 - y * y).sqrt();
    let a = i as f64 * golden;
    [rad * a.cos(), y, rad * a.sin()]
}

pub fn angle_delta(a: f64, b: f64) -> f64 {
    (a - b).sin().atan2((a - b).cos())
}

#[derive(Debug, Clone, Copy)]
pub struct Projector {
    st: f64,
    ct: f64,
    sy: f64,
    cy_: f64,
    cx: f64,
    cy: f64,
    scale: f64,
}

impl Projector {
    pub fn new(yaw: f64, tilt: f64, cx: f64, cy: f64, scale: f64) -> Self {
        Self { st: tilt.sin(), ct: tilt.cos(), sy: yaw.sin(), cy_: yaw.cos(), cx, cy, scale }
    }

    pub fn project(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let x1 = x * self.cy_ + z * self.sy;
        let z1 = -x * self.sy + z * self.cy_;
        let y1 = y * self.ct - z1 * self.st;
        let z2 = y * self.st + z1 * self.ct;
        (self.cx + x1 * self.scale, self.cy - y1 * self.scale, z2)
    }
}

pub fn radius_scale(size: f64, pow: f64) -> f64 {
    (size / 300.0).powf(pow)
}

pub fn finalize_frame(dots: Vec<Dot>, lines: Vec<Line>, r_min: f64) -> OrbFrame {
    let mut visible: Vec<Dot> =
        dots.into_iter().filter(|d| d.a >= 0.02).map(|d| Dot { r: d.r.max(r_min), ..d }).collect();
    visible.sort_by(|a, b| a.z.total_cmp(&b.z));
    OrbFrame { dots: visible, lines: lines.into_iter().filter(|l| l.a >= 0.02).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic_and_unit() {
        let h = hash_d(3.0, 1.7);
        assert_eq!(h, hash_d(3.0, 1.7));
        assert!((0.0..1.0).contains(&h));
    }

    #[test]
    fn noise_is_continuous() {
        let a = vnoise(1.5, 2.5);
        let b = vnoise(1.5001, 2.5);
        assert!((a - b).abs() < 1e-3);
    }

    #[test]
    fn fib_directions_are_unit() {
        for i in 0..20 {
            let [x, y, z] = fib_dir(i, 20);
            assert!(((x * x + y * y + z * z).sqrt() - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn angle_delta_wraps() {
        assert!((angle_delta(0.1, std::f64::consts::TAU - 0.1) - 0.2).abs() < 1e-9);
    }

    #[test]
    fn finalize_sorts_far_to_near_and_clamps() {
        let dot = |z: f64, r: f64, a: f64| Dot { x: 0.0, y: 0.0, z, r, white: 0.5, a };
        let frame = finalize_frame(vec![dot(1.0, 0.1, 1.0), dot(-1.0, 2.0, 1.0), dot(0.0, 1.0, 0.0)], vec![], 0.3);
        assert_eq!(frame.dots.len(), 2);
        assert_eq!(frame.dots[0].z, -1.0);
        assert_eq!(frame.dots[1].r, 0.3);
    }
}
