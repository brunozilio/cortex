//! The app-facing facade: an on/off switch and a throttle in front of an
//! actuator.

use crate::{Actuator, Haptic, Trackpad};

#[derive(Debug)]
pub struct Haptics {
    enabled: bool,
    gap: f64,
    last: Option<f64>,
    actuator: Box<dyn Actuator>,
}

impl Haptics {
    pub fn trackpad(enabled: bool, gap: f64, burst: f64) -> Self {
        Self::with(Box::new(Trackpad::open(burst)), enabled, gap)
    }

    pub fn with(actuator: Box<dyn Actuator>, enabled: bool, gap: f64) -> Self {
        Self { enabled, gap, last: None, actuator }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn feel(&mut self, haptic: Haptic, now: f64) -> bool {
        if !self.enabled || self.last.is_some_and(|last| now - last < self.gap) {
            return false;
        }
        self.last = Some(now);
        self.actuator.perform(haptic);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Recorder;

    fn haptics(enabled: bool) -> (Haptics, Recorder) {
        let recorder = Recorder::default();
        (Haptics::with(Box::new(recorder.clone()), enabled, 0.05), recorder)
    }

    #[test]
    fn pulses_play_their_pattern() {
        let (mut h, rec) = haptics(true);
        assert!(h.feel(Haptic::Lift, 0.0));
        assert!(h.feel(Haptic::Align, 1.0));
        assert!(h.feel(Haptic::Level, 2.0));
        assert_eq!(rec.performed(), vec![Haptic::Lift, Haptic::Align, Haptic::Level]);
    }

    #[test]
    fn pulses_too_close_together_are_dropped() {
        let (mut h, rec) = haptics(true);
        assert!(h.feel(Haptic::Align, 1.0));
        assert!(!h.feel(Haptic::Align, 1.02));
        assert!(h.feel(Haptic::Align, 1.06));
        assert_eq!(rec.performed().len(), 2);
    }

    #[test]
    fn off_plays_nothing_until_turned_back_on() {
        let (mut h, rec) = haptics(false);
        assert!(!h.feel(Haptic::Contact, 0.0));
        h.set_enabled(true);
        assert!(h.feel(Haptic::Contact, 0.0));
        assert_eq!(rec.performed(), vec![Haptic::Contact]);
    }

    #[test]
    fn bumping_strikes_hardest_and_frequent_pulses_lightest() {
        use crate::Strike;
        assert_eq!(Haptic::Contact.strike(), Strike::FIRM);
        assert_eq!(Haptic::Align.strike(), Strike::LIGHT);
        assert_eq!(Haptic::Level.strike(), Strike::LIGHT);
        let valid = |a: i32| (1..=6).contains(&a) || a == 15 || a == 16;
        assert!(Haptic::ALL.iter().all(|h| valid(h.strike().actuation) && h.strike().repeats >= 1));
    }
}
