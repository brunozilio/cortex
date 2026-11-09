//! Damped springs for interruptible, velocity-preserving motion.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl SpringConfig {
    pub const GOO: SpringConfig = SpringConfig { stiffness: 420.0, damping: 30.0, mass: 1.0 };
    pub const LAYOUT: SpringConfig = SpringConfig { stiffness: 260.0, damping: 32.0, mass: 1.0 };
    pub const SOFT: SpringConfig = SpringConfig { stiffness: 140.0, damping: 24.0, mass: 1.0 };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub config: SpringConfig,
}

const MAX_STEP: f32 = 1.0 / 240.0;
const REST_DELTA: f32 = 0.01;
const REST_VELOCITY: f32 = 0.05;

impl Spring {
    pub fn new(value: f32, config: SpringConfig) -> Self {
        Self { value, velocity: 0.0, target: value, config }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    pub fn step(&mut self, dt: f32) {
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return;
        }
        let mut remaining = dt.max(0.0);
        while remaining > 0.0 {
            let h = remaining.min(MAX_STEP);
            let force = -self.config.stiffness * (self.value - self.target) - self.config.damping * self.velocity;
            self.velocity += force / self.config.mass * h;
            self.value += self.velocity * h;
            remaining -= h;
        }
    }

    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < REST_DELTA && self.velocity.abs() < REST_VELOCITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(spring: &mut Spring, seconds: f32) {
        let frames = (seconds * 60.0) as usize;
        for _ in 0..frames {
            spring.step(1.0 / 60.0);
        }
    }

    #[test]
    fn converges_to_target() {
        let mut spring = Spring::new(0.0, SpringConfig::GOO);
        spring.set_target(100.0);
        run(&mut spring, 2.0);
        assert!(spring.is_settled());
        assert!((spring.value - 100.0).abs() < 0.05);
    }

    #[test]
    fn layout_spring_does_not_overshoot_much() {
        let mut spring = Spring::new(0.0, SpringConfig::LAYOUT);
        spring.set_target(1.0);
        let mut peak = 0.0_f32;
        for _ in 0..120 {
            spring.step(1.0 / 60.0);
            peak = peak.max(spring.value);
        }
        assert!(peak < 1.02, "peak {peak}");
    }

    #[test]
    fn frame_rate_independent() {
        let mut a = Spring::new(0.0, SpringConfig::GOO);
        let mut b = a;
        a.set_target(1.0);
        b.set_target(1.0);
        for _ in 0..30 {
            a.step(1.0 / 60.0);
        }
        for _ in 0..60 {
            b.step(1.0 / 120.0);
        }
        assert!((a.value - b.value).abs() < 1e-3);
    }

    #[test]
    fn snap_stops_motion() {
        let mut spring = Spring::new(0.0, SpringConfig::SOFT);
        spring.set_target(5.0);
        spring.step(0.016);
        spring.snap(2.0);
        assert_eq!((spring.value, spring.velocity, spring.target), (2.0, 0.0, 2.0));
    }
}
