//! Two fingers swiped sideways over a page step through its history, as in
//! Chromium browsers: fingers to the right go back, to the left forward.
//! Pure and tested; the shell feeds trackpad wheel travel and dispatches.

use superfik_ui::theme::{metrics, motion};
use winit::event::TouchPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swiped {
    Back,
    Forward,
}

#[derive(Debug, Default)]
pub struct PageSwipe {
    dx: f32,
    dy: f32,
    fired_at: Option<f64>,
}

impl PageSwipe {
    pub fn feed(&mut self, phase: TouchPhase, dx: f32, dy: f32, now: f64) -> Option<Swiped> {
        if phase == TouchPhase::Started {
            (self.dx, self.dy) = (0.0, 0.0);
        }
        self.dx += dx;
        self.dy += dy;
        let resting = self.fired_at.is_some_and(|at| now - at < motion::PAGE_SWIPE_REST);
        let sideways = self.dx.abs() >= metrics::PAGE_SWIPE_TRAVEL && self.dx.abs() > self.dy.abs() * 2.0;
        if resting || !sideways {
            return None;
        }
        self.fired_at = Some(now);
        let swiped = if self.dx > 0.0 { Swiped::Back } else { Swiped::Forward };
        (self.dx, self.dy) = (0.0, 0.0);
        Some(swiped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAR: f32 = metrics::PAGE_SWIPE_TRAVEL;

    #[test]
    fn a_sideways_swipe_steps_once_it_travels_far_enough() {
        let mut swipe = PageSwipe::default();
        assert_eq!(swipe.feed(TouchPhase::Started, FAR * 0.5, 0.0, 0.0), None);
        assert_eq!(swipe.feed(TouchPhase::Moved, FAR * 0.6, 0.0, 0.1), Some(Swiped::Back));
        assert_eq!(swipe.feed(TouchPhase::Moved, FAR * 2.0, 0.0, 0.2), None, "one step per swipe");
    }

    #[test]
    fn fingers_to_the_left_go_forward() {
        let mut swipe = PageSwipe::default();
        assert_eq!(swipe.feed(TouchPhase::Started, -FAR * 1.2, 0.0, 0.0), Some(Swiped::Forward));
    }

    #[test]
    fn scrolling_down_a_page_never_steps() {
        let mut swipe = PageSwipe::default();
        assert_eq!(swipe.feed(TouchPhase::Started, FAR, FAR, 0.0), None, "mostly down");
    }

    #[test]
    fn the_momentum_after_a_swipe_takes_no_second_step() {
        let mut swipe = PageSwipe::default();
        swipe.feed(TouchPhase::Started, FAR * 1.2, 0.0, 0.0);
        let rest = motion::PAGE_SWIPE_REST;
        assert_eq!(swipe.feed(TouchPhase::Started, FAR * 1.2, 0.0, rest * 0.5), None);
        assert_eq!(swipe.feed(TouchPhase::Started, FAR * 1.2, 0.0, rest + 0.1), Some(Swiped::Back));
    }

    #[test]
    fn a_new_gesture_starts_from_nothing() {
        let mut swipe = PageSwipe::default();
        swipe.feed(TouchPhase::Started, FAR * 0.8, 0.0, 0.0);
        assert_eq!(swipe.feed(TouchPhase::Started, FAR * 0.8, 0.0, 1.0), None);
    }
}
