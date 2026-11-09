//! The update screen over the whole window: the emote, a title (lit by a
//! passing sweep while updating), a line on what is going on, the progress
//! bar and its stage, and a button (Update, Try again, Restart to update)
//! with a quieter one under it. Stateless: the caller says what each part
//! reads and whether it shows.

use superfik_render::{Point, Quad, Rect};

use super::{BarProgress, Button, Emote, ProgressBar};
use crate::painter::Painter;
use crate::theme::{metrics, motion, typography};

#[derive(Debug, Clone, Copy)]
pub struct UpdateScreen<'a> {
    pub emote: Emote,
    pub title: &'a str,
    pub shimmer: Option<f32>,
    pub caption: &'a str,
    pub progress: Option<BarProgress>,
    pub status: Option<&'a str>,
    pub primary: Option<Button<'a>>,
    pub secondary: Option<Button<'a>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UpdateScreenHits {
    pub primary: Option<Rect>,
    pub secondary: Option<Rect>,
}

pub fn sweep(at: f32, time: f32) -> f32 {
    const BAND: f32 = 0.35;
    let centre = (time / motion::UPDATE_SHIMMER).fract() * (1.0 + 2.0 * BAND) - BAND;
    (1.0 - ((at - centre).abs() / BAND)).clamp(0.0, 1.0)
}

impl UpdateScreen<'_> {
    pub fn paint(&self, p: &mut Painter, area: Rect) -> UpdateScreenHits {
        let c = p.palette();
        p.quad(Quad::new(area, 0.0, c.canvas));
        let width = metrics::UPDATE_WIDTH.min(area.w - metrics::SCREEN_PADDING * 2.0).max(0.0);
        let title = p.layout(self.title, typography::UPDATE_TITLE, None);
        let caption = p.layout(self.caption, typography::UPDATE_CAPTION, Some(width));
        let status = self.status.map(|status| p.layout(status, typography::CAPTION, None));
        let gap = metrics::UPDATE_GAP;
        let bar_h = self.progress.map_or(0.0, |_| gap * 2.0 + metrics::UPDATE_BAR);
        let status_h = status.as_ref().map_or(0.0, |status| gap + status.height);
        let buttons = self.primary.map_or(0.0, |_| gap * 2.0 + metrics::CHIP_HEIGHT)
            + self.secondary.map_or(0.0, |_| gap * 0.5 + metrics::CHIP_HEIGHT);
        let height = metrics::UPDATE_EMOTE + gap * 2.0 + title.height + gap * 0.5 + caption.height + bar_h + status_h;
        let cx = area.center().x;
        let mut y = area.y + ((area.h - height - buttons) * 0.5).max(metrics::SCREEN_PADDING);
        let emote = Emote { width: metrics::UPDATE_EMOTE, ..self.emote };
        emote.paint(p, Point::new(cx, y + metrics::UPDATE_EMOTE * 0.5));
        y += metrics::UPDATE_EMOTE + gap * 2.0;
        let origin = Point::new(cx - title.width * 0.5, y);
        match self.shimmer {
            Some(time) => {
                let len = self.title.len().max(1) as f32;
                let (dim, lit) = (c.text_tertiary, c.text);
                p.text_with(&title, origin, |at| (dim.mix(lit, sweep(at as f32 / len, time)), 0.0));
            }
            None => p.text(&title, origin, c.text),
        }
        y += title.height + gap * 0.5;
        p.text_centered(&caption, cx, y, c.text_secondary);
        y += caption.height;
        if let Some(progress) = self.progress {
            y += gap * 2.0;
            ProgressBar { progress }.paint(p, Rect::new(cx - width * 0.5, y, width, metrics::UPDATE_BAR));
            y += metrics::UPDATE_BAR;
        }
        if let Some(status) = &status {
            y += gap;
            p.text_centered(status, cx, y, c.text_secondary);
            y += status.height;
        }
        let mut hits = UpdateScreenHits::default();
        if let Some(button) = self.primary {
            y += gap * 2.0;
            let w = button.width(p);
            hits.primary = Some(button.paint(p, Rect::new(cx - w * 0.5, y, w, metrics::CHIP_HEIGHT)));
            y += metrics::CHIP_HEIGHT;
        }
        if let Some(button) = self.secondary {
            y += gap * 0.5;
            let w = button.width(p);
            hits.secondary = Some(button.paint(p, Rect::new(cx - w * 0.5, y, w, metrics::CHIP_HEIGHT)));
        }
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_crosses_the_title_once_per_period() {
        let quarter = motion::UPDATE_SHIMMER * 0.25;
        let half = motion::UPDATE_SHIMMER * 0.5;
        assert!(sweep(0.5, half) > 0.99, "the middle is lit halfway through");
        assert!(sweep(0.0, half) < sweep(0.5, half));
        assert!(sweep(0.1, quarter) > sweep(0.9, quarter), "it moves left to right");
        assert_eq!(sweep(0.5, 0.0), 0.0, "it starts before the title");
    }
}
