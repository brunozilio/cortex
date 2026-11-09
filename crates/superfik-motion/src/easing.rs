//! Easing curves, including CSS-style cubic béziers.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    pub const OUT: Easing = Easing::CubicBezier(0.22, 1.0, 0.36, 1.0);
    pub const IN_OUT: Easing = Easing::CubicBezier(0.65, 0.0, 0.35, 1.0);
    pub const IN: Easing = Easing::CubicBezier(0.55, 0.0, 1.0, 0.45);

    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::CubicBezier(x1, y1, x2, y2) => cubic_bezier(t, x1, y1, x2, y2),
        }
    }
}

fn bezier(t: f32, p1: f32, p2: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
}

fn bezier_slope(t: f32, p1: f32, p2: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * p1 + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

fn cubic_bezier(x: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let mut s = x;
    for _ in 0..8 {
        let error = bezier(s, x1, x2) - x;
        if error.abs() < 1e-5 {
            return bezier(s, y1, y2);
        }
        let slope = bezier_slope(s, x1, x2);
        if slope.abs() < 1e-6 {
            break;
        }
        s -= error / slope;
    }
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    s = x;
    for _ in 0..32 {
        let value = bezier(s, x1, x2);
        if (value - x).abs() < 1e-5 {
            break;
        }
        if value < x {
            lo = s
        } else {
            hi = s
        }
        s = (lo + hi) * 0.5;
    }
    bezier(s, y1, y2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_fixed() {
        for easing in [Easing::Linear, Easing::OUT, Easing::IN_OUT, Easing::IN] {
            assert!((easing.apply(0.0)).abs() < 1e-4);
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn ease_out_leads_linear() {
        assert!(Easing::OUT.apply(0.3) > 0.3);
        assert!(Easing::IN.apply(0.3) < 0.3);
    }

    #[test]
    fn symmetric_curve_crosses_half_at_half() {
        assert!((Easing::IN_OUT.apply(0.5) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn linear_bezier_is_identity() {
        let linear = Easing::CubicBezier(0.25, 0.25, 0.75, 0.75);
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            assert!((linear.apply(t) - t).abs() < 1e-3);
        }
    }

    #[test]
    fn input_is_clamped() {
        assert_eq!(Easing::Linear.apply(2.0), 1.0);
        assert_eq!(Easing::Linear.apply(-1.0), 0.0);
    }
}
