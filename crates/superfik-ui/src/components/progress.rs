//! A thin progress bar: a track and its fill, or, when how far is not
//! known, a segment passing along the track. Stateless: the caller passes
//! the share done or the clock.

use superfik_render::{Quad, Rect};

use crate::painter::Painter;
use crate::theme::{metrics, motion};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarProgress {
    Known(f32),
    Unknown { time: f32 },
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressBar {
    pub progress: BarProgress,
}

impl ProgressBar {
    pub fn span(progress: BarProgress) -> (f32, f32) {
        match progress {
            BarProgress::Known(done) => (0.0, done.clamp(0.0, 1.0)),
            BarProgress::Unknown { time } => {
                let segment = metrics::UPDATE_BAR_SEGMENT;
                let pass = (time / motion::UPDATE_BAR_PASS).fract();
                let head = pass * (1.0 + segment);
                ((head - segment).max(0.0), head.min(1.0))
            }
        }
    }

    pub fn paint(&self, p: &mut Painter, rect: Rect) {
        let c = p.palette();
        let h = metrics::UPDATE_BAR;
        let track = Rect::new(rect.x, rect.center().y - h * 0.5, rect.w, h);
        p.quad(Quad::new(track, h * 0.5, c.well));
        let (from, to) = Self::span(self.progress);
        if to - from > 0.0005 {
            let fill = Rect::new(track.x + track.w * from, track.y, track.w * (to - from), h);
            p.quad(Quad::new(fill, h * 0.5, c.form_accent).clip(track));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_share_fills_from_the_left_and_stays_inside() {
        assert_eq!(ProgressBar::span(BarProgress::Known(0.4)), (0.0, 0.4));
        assert_eq!(ProgressBar::span(BarProgress::Known(1.7)), (0.0, 1.0));
    }

    #[test]
    fn an_unknown_one_passes_a_segment_along_the_track() {
        let (from, to) = ProgressBar::span(BarProgress::Unknown { time: 0.0 });
        assert_eq!((from, to), (0.0, 0.0), "it enters from the left");
        let middle = ProgressBar::span(BarProgress::Unknown { time: motion::UPDATE_BAR_PASS * 0.5 });
        assert!(middle.0 > 0.0 && middle.1 < 1.0 && middle.1 > middle.0);
        let late = ProgressBar::span(BarProgress::Unknown { time: motion::UPDATE_BAR_PASS * 0.99 });
        assert!(late.1 == 1.0 && late.0 > 0.7, "and leaves past the right: {late:?}");
    }
}
