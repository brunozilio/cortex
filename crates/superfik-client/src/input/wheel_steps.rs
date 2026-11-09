//! Turns wheel travel into steps, like detents: sessions on the timeline
//! (one per row of travel), pages of the Skills carousel. Pure and
//! tested; the shell dispatches.

use crate::state::Toward;

#[derive(Debug, Default)]
pub struct WheelSteps {
    pending: f32,
}

impl WheelSteps {
    pub fn feed(&mut self, dy: f32, detent: f32) -> Option<(Toward, usize)> {
        if self.pending * dy < 0.0 {
            self.pending = 0.0;
        }
        self.pending += dy;
        let rows = (self.pending.abs() / detent).floor();
        if rows < 1.0 {
            return None;
        }
        let toward = if self.pending < 0.0 { Toward::Older } else { Toward::Newer };
        self.pending -= rows * detent * self.pending.signum();
        Some((toward, rows as usize))
    }

    pub fn reset(&mut self) {
        self.pending = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use superfik_ui::theme::metrics;

    use super::*;

    const ROW: f32 = metrics::TIMELINE_ROW;

    #[test]
    fn small_deltas_add_up_to_a_step() {
        let mut steps = WheelSteps::default();
        assert_eq!(steps.feed(-ROW * 0.6, ROW), None);
        assert_eq!(steps.feed(-ROW * 0.6, ROW), Some((Toward::Older, 1)));
        assert_eq!(steps.feed(-ROW * 0.8, ROW), Some((Toward::Older, 1)), "the remainder carries over");
    }

    #[test]
    fn a_long_flick_crosses_several_rows() {
        let mut steps = WheelSteps::default();
        assert_eq!(steps.feed(ROW * 3.2, ROW), Some((Toward::Newer, 3)));
    }

    #[test]
    fn turning_back_drops_the_partial_travel() {
        let mut steps = WheelSteps::default();
        steps.feed(-ROW * 0.9, ROW);
        assert_eq!(steps.feed(ROW * 0.5, ROW), None);
        assert_eq!(steps.feed(ROW * 0.5, ROW), Some((Toward::Newer, 1)));
    }

    #[test]
    fn reset_forgets_partial_travel() {
        let mut steps = WheelSteps::default();
        steps.feed(-ROW * 0.9, ROW);
        steps.reset();
        assert_eq!(steps.feed(-ROW * 0.5, ROW), None);
    }
}
