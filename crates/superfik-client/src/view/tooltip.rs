//! Tooltips for shortened labels: resting the pointer on a label that ends
//! in "…" shows its whole text.
//!
//! Each paint pass hands over the labels it shortened ([`Elided`]); the
//! topmost one under the pointer, not covered by anything painted after
//! it, is the one to explain. [`TooltipHover`] times the rest before the
//! tooltip fades in.

use superfik_render::{Point, Rect};
use superfik_ui::components::Tooltip;
use superfik_ui::theme::motion;
use superfik_ui::{Elided, Painter, Plane};

use super::target::HitMap;

#[derive(Debug, Default)]
pub struct ElidedLabels {
    labels: Vec<(Elided, usize)>,
}

impl ElidedLabels {
    pub fn add(&mut self, labels: Vec<Elided>, screen_hits: usize) {
        self.labels.extend(labels.into_iter().map(|label| (label, screen_hits)));
    }

    pub fn under(&self, pointer: Point, hits: &HitMap) -> Option<&str> {
        let (label, from) = self.labels.iter().rev().find(|(label, _)| label.rect.contains(pointer))?;
        (!hits.screen_covers_from(*from, pointer)).then_some(label.text.as_str())
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TooltipHover {
    text: Option<String>,
    since: f64,
    dismissed: bool,
}

impl TooltipHover {
    pub fn sync(&mut self, under: Option<&str>, now: f64) {
        if self.text.as_deref() != under {
            self.text = under.map(str::to_owned);
            self.since = now;
            self.dismissed = false;
        }
    }

    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }

    pub fn showing(&self, now: f64) -> Option<(&str, f32)> {
        let text = self.text.as_deref().filter(|_| !self.dismissed)?;
        let rested = now - self.since - motion::TOOLTIP_DELAY;
        (rested >= 0.0).then(|| (text, (rested as f32 / motion::HOVER).min(1.0)))
    }

    pub fn paint(&self, p: &mut Painter, pointer: Point, viewport: (f32, f32), now: f64) {
        let Some((text, progress)) = self.showing(now) else { return };
        let viewport = Rect::new(0.0, 0.0, viewport.0, viewport.1);
        p.on_plane(Plane::Overlay, |p| Tooltip { text, pointer, viewport, progress }.paint(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::target::Target;

    fn label(x: f32, text: &str) -> Elided {
        Elided { rect: Rect::new(x, 0.0, 50.0, 20.0), text: text.to_owned() }
    }

    #[test]
    fn the_topmost_label_under_the_pointer_wins() {
        let mut labels = ElidedLabels::default();
        labels.add(vec![label(0.0, "under"), label(20.0, "over")], 0);
        let hits = HitMap::default();
        assert_eq!(labels.under(Point::new(30.0, 10.0), &hits), Some("over"));
        assert_eq!(labels.under(Point::new(5.0, 10.0), &hits), Some("under"));
        assert_eq!(labels.under(Point::new(5.0, 50.0), &hits), None);
    }

    #[test]
    fn a_label_covered_by_a_later_pass_has_no_tooltip() {
        let mut hits = HitMap::default();
        let mut labels = ElidedLabels::default();
        labels.add(vec![label(0.0, "canvas")], hits.screen_len());
        hits.add_screen(Rect::new(0.0, 0.0, 10.0, 20.0), Target::Surface);
        labels.add(vec![label(100.0, "sidebar")], hits.screen_len());
        assert_eq!(labels.under(Point::new(5.0, 10.0), &hits), None, "the sidebar covers it");
        assert_eq!(labels.under(Point::new(30.0, 10.0), &hits), Some("canvas"));
        assert_eq!(labels.under(Point::new(110.0, 10.0), &hits), Some("sidebar"));
    }

    #[test]
    fn shows_after_resting_then_fades_in() {
        let mut hover = TooltipHover::default();
        hover.sync(Some("whole text"), 1.0);
        assert_eq!(hover.showing(1.0 + motion::TOOLTIP_DELAY * 0.5), None);
        let (text, faded) = hover.showing(1.0 + motion::TOOLTIP_DELAY).expect("shows");
        assert_eq!((text, faded), ("whole text", 0.0));
        hover.sync(Some("whole text"), 9.0);
        assert_eq!(hover.showing(9.0), Some(("whole text", 1.0)), "staying on it keeps it");
    }

    #[test]
    fn another_label_restarts_the_wait_and_leaving_hides_it() {
        let mut hover = TooltipHover::default();
        hover.sync(Some("a"), 0.0);
        hover.sync(Some("b"), 5.0);
        assert_eq!(hover.showing(5.1), None);
        hover.sync(None, 9.0);
        assert_eq!(hover.showing(20.0), None);
    }

    #[test]
    fn a_click_hides_it_until_the_next_label() {
        let mut hover = TooltipHover::default();
        hover.sync(Some("a"), 0.0);
        hover.dismiss();
        hover.sync(Some("a"), 5.0);
        assert_eq!(hover.showing(5.0), None);
        hover.sync(Some("b"), 6.0);
        assert!(hover.showing(6.0 + motion::TOOLTIP_DELAY).is_some());
    }
}
