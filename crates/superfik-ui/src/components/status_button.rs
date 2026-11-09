//! A pill button that shows how what it started is going: its icon and
//! label at rest, a spinner and a busy label while it works, a check and a
//! done label on a success tint once it is done. Between stages its width
//! eases to the new content while the old content blurs out and the new
//! blurs in (transitions.dev's card resize and text morph); the check's
//! stroke draws itself. Done holds a few seconds, long enough to read,
//! then eases back to rest the same way (transitions.dev's auto-revert).

use superfik_motion::Easing;
use superfik_render::{Color, GooShape, Icon, Point, Quad, Rect};

use super::browser_glyphs::spinner;
use crate::painter::{Painter, Plane};
use crate::theme::{metrics, motion, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    Idle,
    Busy { age: f32, time: f32 },
    Done { age: f32, after_busy: bool },
}

impl Progress {
    pub fn is_moving(&self) -> bool {
        match *self {
            Progress::Idle => false,
            Progress::Busy { .. } => true,
            Progress::Done { age, .. } => age < motion::STATUS_DONE_HOLD + motion::STATUS_REVERT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    Busy,
    Done,
}

#[derive(Debug, Clone, Copy)]
pub struct StatusButton<'a> {
    pub labels: [&'a str; 3],
    pub icon: Icon,
    pub progress: Progress,
    pub hovered: bool,
    pub enabled: bool,
}

const CHECK: [(f32, f32); 3] = [(-0.34, 0.02), (-0.1, 0.27), (0.36, -0.25)];

impl StatusButton<'_> {
    pub fn paint(&self, p: &mut Painter, right: f32, center_y: f32) -> Rect {
        let (from, to, t, age) = self.stages();
        let morph = Easing::OUT.apply(t);
        let (w_from, w_to) = (self.width(p, from), self.width(p, to));
        let h = metrics::STATUS_BUTTON_HEIGHT;
        let w = w_from + (w_to - w_from) * morph;
        let rect = Rect::new(right - w, (center_y - h * 0.5).round(), w, h);
        let c = p.palette();
        for (stage, opacity) in [(from, 1.0), (to, morph)] {
            let fill = match stage {
                Stage::Idle if self.hovered && self.enabled => c.hover,
                Stage::Idle | Stage::Busy => c.well,
                Stage::Done => c.success.faded(metrics::STATUS_DONE_TINT),
            };
            let mut quad = Quad::new(rect, h * 0.5, fill).opacity(opacity);
            if stage == Stage::Done {
                quad = quad.border(c.success.faded(metrics::STATUS_DONE_BORDER), 1.0);
            }
            p.quad(quad);
        }
        p.clipped(rect, |p| {
            if from != to {
                p.faded(1.0 - morph, |p| p.blurred(morph * motion::BLUR, |p| self.content(p, rect, from, age)));
            }
            p.faded(morph, |p| p.blurred((1.0 - morph) * motion::BLUR, |p| self.content(p, rect, to, age)));
        });
        rect
    }

    fn stages(&self) -> (Stage, Stage, f32, f32) {
        let over = |age: f32| (age / motion::STATUS_MORPH).clamp(0.0, 1.0);
        match self.progress {
            Progress::Idle => (Stage::Idle, Stage::Idle, 1.0, f32::MAX),
            Progress::Busy { age, .. } => (Stage::Idle, Stage::Busy, over(age), age),
            Progress::Done { age, after_busy } if age < motion::STATUS_DONE_HOLD => {
                let from = if after_busy { Stage::Busy } else { Stage::Idle };
                (from, Stage::Done, over(age), age)
            }
            Progress::Done { age, .. } if self.progress.is_moving() => {
                let back = ((age - motion::STATUS_DONE_HOLD) / motion::STATUS_REVERT).clamp(0.0, 1.0);
                (Stage::Done, Stage::Idle, back, age)
            }
            Progress::Done { .. } => (Stage::Idle, Stage::Idle, 1.0, f32::MAX),
        }
    }

    fn label(&self, stage: Stage) -> &str {
        match stage {
            Stage::Idle => self.labels[0],
            Stage::Busy => self.labels[1],
            Stage::Done => self.labels[2],
        }
    }

    fn width(&self, p: &mut Painter, stage: Stage) -> f32 {
        let label = p.layout(self.label(stage), typography::CHIP, None).width;
        metrics::STATUS_BUTTON_PADDING * 2.0 + metrics::ICON + metrics::STATUS_BUTTON_GAP + label
    }

    fn content(&self, p: &mut Painter, rect: Rect, stage: Stage, age: f32) {
        let c = p.palette();
        let layout = p.layout(self.label(stage), typography::CHIP, None);
        let inner = metrics::ICON + metrics::STATUS_BUTTON_GAP + layout.width;
        let x = rect.center().x - inner * 0.5;
        let cy = rect.center().y;
        let glyph = Point::new(x + metrics::ICON * 0.5, cy);
        let ink = match stage {
            Stage::Idle if self.enabled => c.text_body,
            Stage::Idle | Stage::Busy => c.text_secondary,
            Stage::Done => c.success,
        };
        match stage {
            Stage::Idle => p.icon_at(self.icon, glyph, metrics::ICON, Some(ink)),
            Stage::Busy => {
                let time = match self.progress {
                    Progress::Busy { time, .. } => time,
                    _ => 0.0,
                };
                p.on_plane(Plane::Overlay, |p| spinner(p, glyph, metrics::ICON, time, ink));
            }
            Stage::Done => {
                let drawn = Easing::OUT.apply((age - motion::SUCCESS_DRAW_DELAY) / motion::SUCCESS_CHECK);
                p.on_plane(Plane::Overlay, |p| check(p, glyph, metrics::ICON, drawn, ink));
            }
        }
        let at = Point::new(x + metrics::ICON + metrics::STATUS_BUTTON_GAP, (cy - layout.height * 0.5).round());
        p.text(&layout, at, ink);
    }
}

fn check(p: &mut Painter, center: Point, size: f32, drawn: f32, color: Color) {
    let points = CHECK.map(|(x, y)| Point::new(center.x + x * size, center.y + y * size));
    let lengths = [distance(points[0], points[1]), distance(points[1], points[2])];
    let mut left = (lengths[0] + lengths[1]) * drawn.clamp(0.0, 1.0);
    for (w, length) in points.windows(2).zip(lengths) {
        if left <= 0.0 {
            break;
        }
        let k = (left / length).min(1.0);
        let end = Point::new(w[0].x + (w[1].x - w[0].x) * k, w[0].y + (w[1].y - w[0].y) * k);
        p.well(GooShape::Capsule { a: w[0], b: end, radius: metrics::GLYPH_STROKE * 0.5 }, color, 0.0);
        left -= length;
    }
}

fn distance(a: Point, b: Point) -> f32 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stages(progress: Progress) -> (Stage, Stage, f32) {
        let button = StatusButton {
            labels: ["Import", "Importing", "Imported"],
            icon: crate::icons::IMPORT,
            progress,
            hovered: false,
            enabled: true,
        };
        let (from, to, t, _) = button.stages();
        (from, to, t)
    }

    #[test]
    fn each_stage_morphs_from_the_one_before() {
        assert_eq!(stages(Progress::Idle), (Stage::Idle, Stage::Idle, 1.0));
        let busy = stages(Progress::Busy { age: motion::STATUS_MORPH * 0.5, time: 0.0 });
        assert_eq!((busy.0, busy.1), (Stage::Idle, Stage::Busy));
        assert!((busy.2 - 0.5).abs() < 1e-5);
        assert_eq!(stages(Progress::Done { age: 1.0, after_busy: true }), (Stage::Busy, Stage::Done, 1.0));
        assert_eq!(stages(Progress::Done { age: 0.0, after_busy: false }).0, Stage::Idle, "cleared at once");
    }

    #[test]
    fn done_holds_then_morphs_back_to_rest() {
        let hold = motion::STATUS_DONE_HOLD;
        assert_eq!(stages(Progress::Done { age: hold - 0.01, after_busy: true }), (Stage::Busy, Stage::Done, 1.0));
        let back = stages(Progress::Done { age: hold + motion::STATUS_REVERT * 0.5, after_busy: true });
        assert_eq!((back.0, back.1), (Stage::Done, Stage::Idle));
        assert!((back.2 - 0.5).abs() < 1e-4);
        let rest = stages(Progress::Done { age: hold + motion::STATUS_REVERT, after_busy: true });
        assert_eq!(rest, (Stage::Idle, Stage::Idle, 1.0));
        assert!(Progress::Done { age: hold, after_busy: true }.is_moving());
        assert!(!Progress::Done { age: hold + motion::STATUS_REVERT, after_busy: true }.is_moving());
        assert!(!Progress::Idle.is_moving() && Progress::Busy { age: 9.0, time: 0.0 }.is_moving());
    }
}
