//! Enter/exit presence for elements that mount and unmount with motion.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    visible: bool,
    origin: f32,
    changed_at: f64,
    enter: f32,
    exit: f32,
}

impl Presence {
    pub fn new(visible: bool, enter: f32, exit: f32) -> Self {
        Self { visible, origin: if visible { 1.0 } else { 0.0 }, changed_at: f64::NEG_INFINITY, enter, exit }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set(&mut self, visible: bool, now: f64) {
        if visible == self.visible {
            return;
        }
        self.origin = self.progress(now);
        self.visible = visible;
        self.changed_at = now;
    }

    pub fn progress(&self, now: f64) -> f32 {
        let elapsed = (now - self.changed_at).max(0.0) as f32;
        if self.visible {
            let span = self.enter.max(f32::EPSILON) * (1.0 - self.origin);
            if span <= f32::EPSILON { 1.0 } else { (self.origin + (1.0 - self.origin) * elapsed / span).min(1.0) }
        } else {
            let span = self.exit.max(f32::EPSILON) * self.origin;
            if span <= f32::EPSILON { 0.0 } else { (self.origin - self.origin * elapsed / span).max(0.0) }
        }
    }

    pub fn elapsed(&self, now: f64) -> f32 {
        (now - self.changed_at).max(0.0) as f32
    }

    pub fn is_mounted(&self, now: f64) -> bool {
        self.visible || self.progress(now) > 0.0
    }

    pub fn is_animating(&self, now: f64) -> bool {
        let p = self.progress(now);
        if self.visible { p < 1.0 } else { p > 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enters_and_exits() {
        let mut presence = Presence::new(false, 0.5, 0.25);
        assert!(!presence.is_mounted(0.0));
        presence.set(true, 1.0);
        assert_eq!(presence.progress(1.25), 0.5);
        assert_eq!(presence.progress(2.0), 1.0);
        presence.set(false, 2.0);
        assert!((presence.progress(2.125) - 0.5).abs() < 1e-5);
        assert!(!presence.is_mounted(3.0));
    }

    #[test]
    fn reversal_continues_from_current_progress() {
        let mut presence = Presence::new(false, 1.0, 1.0);
        presence.set(true, 0.0);
        presence.set(false, 0.4);
        assert!((presence.progress(0.4) - 0.4).abs() < 1e-5);
        assert!(presence.is_animating(0.5));
        assert_eq!(presence.progress(0.8), 0.0);
    }

    #[test]
    fn initially_visible_is_settled() {
        let presence = Presence::new(true, 0.3, 0.3);
        assert_eq!(presence.progress(0.0), 1.0);
        assert!(!presence.is_animating(0.0));
    }
}
