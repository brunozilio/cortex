//! Interpolation helpers.

pub trait Lerp: Copy {
    fn lerp(self, to: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, to: Self, t: f32) -> Self {
        self + (to - self) * t
    }
}

impl Lerp for [f32; 2] {
    fn lerp(self, to: Self, t: f32) -> Self {
        [self[0].lerp(to[0], t), self[1].lerp(to[1], t)]
    }
}

impl Lerp for [f32; 4] {
    fn lerp(self, to: Self, t: f32) -> Self {
        [self[0].lerp(to[0], t), self[1].lerp(to[1], t), self[2].lerp(to[2], t), self[3].lerp(to[3], t)]
    }
}

pub fn remap(value: f32, start: f32, end: f32) -> f32 {
    if (end - start).abs() < f32::EPSILON {
        return if value >= end { 1.0 } else { 0.0 };
    }
    ((value - start) / (end - start)).clamp(0.0, 1.0)
}

pub fn stagger(t: f32, index: usize, delay: f32, duration: f32) -> f32 {
    remap(t, index as f32 * delay, index as f32 * delay + duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerps() {
        assert_eq!(2.0_f32.lerp(4.0, 0.5), 3.0);
        assert_eq!([0.0, 10.0].lerp([10.0, 20.0], 0.1), [1.0, 11.0]);
    }

    #[test]
    fn remap_clamps_and_handles_empty_range() {
        assert_eq!(remap(5.0, 0.0, 10.0), 0.5);
        assert_eq!(remap(-1.0, 0.0, 10.0), 0.0);
        assert_eq!(remap(11.0, 0.0, 10.0), 1.0);
        assert_eq!(remap(1.0, 1.0, 1.0), 1.0);
    }

    #[test]
    fn stagger_offsets_items() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        assert!(close(stagger(0.05, 0, 0.05, 0.1), 0.5));
        assert!(close(stagger(0.05, 1, 0.05, 0.1), 0.0));
        assert!(close(stagger(0.10, 1, 0.05, 0.1), 0.5));
    }
}
