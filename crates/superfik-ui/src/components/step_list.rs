//! The steps under "Worked for 3s · 4 steps": a quiet list joined by a
//! thin rail, each row an icon, a verb and what it acted on. An opened step
//! shows what its tool returned beneath its row; a failed one is marked.

use superfik_render::{Icon, Point, Quad, Rect};

use super::reply::Reveal;
use crate::painter::Painter;
use crate::theme::{metrics, motion, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepItem<'a> {
    pub icon: Icon,
    pub verb: &'a str,
    pub target: &'a str,
    pub failed: bool,
    pub output: Option<&'a str>,
}

pub struct StepList<'a> {
    pub items: &'a [StepItem<'a>],
    pub width: f32,
    pub reveal: Reveal<'a>,
}

impl std::fmt::Debug for StepList<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StepList").field("items", &self.items).finish()
    }
}

const VERB_GAP: f32 = 6.0;
const OUTPUT_INSET: f32 = 8.0;

pub fn output_height(text: &str) -> f32 {
    let lines = text.lines().count().clamp(1, metrics::STEP_OUTPUT_LINES);
    lines as f32 * metrics::STEP_OUTPUT_LINE + metrics::STEP_OUTPUT_PAD * 2.0
}

impl StepList<'_> {
    pub fn row_height(item: &StepItem) -> f32 {
        Self::row_height_of(item.output)
    }

    pub fn row_height_of(output: Option<&str>) -> f32 {
        metrics::STEP_ROW + output.map_or(0.0, output_height)
    }

    pub fn height(items: &[StepItem]) -> f32 {
        items.iter().map(Self::row_height).sum()
    }

    pub fn paint(&self, p: &mut Painter, origin: Point) -> Vec<Rect> {
        let icon_x = origin.x + metrics::STEP_INSET;
        let mut tops = Vec::with_capacity(self.items.len());
        let mut y = origin.y;
        for item in self.items {
            tops.push(y);
            y += Self::row_height(item);
        }
        if let (true, Some(last)) = (self.items.len() > 1, tops.last()) {
            let top = origin.y + metrics::STEP_ROW * 0.5;
            let bottom = last + metrics::STEP_ROW * 0.5;
            let rail = Rect::new(icon_x - metrics::STEP_RAIL * 0.5, top, metrics::STEP_RAIL, bottom - top);
            p.quad(Quad::new(rail, metrics::STEP_RAIL * 0.5, p.palette().step_rail.faded((self.reveal)(0))));
        }
        for (index, item) in self.items.iter().enumerate() {
            let t = (self.reveal)(index);
            if t <= 0.0 {
                continue;
            }
            let top = tops[index];
            let cy = top + metrics::STEP_ROW * 0.5;
            p.faded(t, |p| {
                p.blurred((1.0 - t) * motion::BLUR, |p| {
                    self.paint_row(p, item, icon_x, cy, origin.x + self.width);
                    if let Some(text) = item.output {
                        let block = Rect::new(
                            icon_x + metrics::ICON_SMALL + 4.0,
                            top + metrics::STEP_ROW,
                            (origin.x + self.width) - (icon_x + metrics::ICON_SMALL + 4.0),
                            output_height(text),
                        );
                        paint_output(p, text, block);
                    }
                })
            });
        }
        tops.iter().map(|top| Rect::new(origin.x, *top, self.width, metrics::STEP_ROW)).collect()
    }

    fn paint_row(&self, p: &mut Painter, item: &StepItem, icon_x: f32, cy: f32, right: f32) {
        let disc = metrics::ICON_SMALL + 4.0;
        p.quad(Quad::new(Rect::new(icon_x - disc * 0.5, cy - disc * 0.5, disc, disc), disc * 0.5, p.palette().canvas));
        let tint = if item.failed { p.palette().danger } else { p.palette().icon_muted };
        p.icon_at(item.icon, Point::new(icon_x, cy), metrics::ICON_SMALL, Some(tint));
        let x = icon_x + metrics::ICON_SMALL + 4.0;
        let verb = p.label(item.verb, typography::LABEL, x, cy, p.palette().text_secondary);
        let target_x = x + verb.width + VERB_GAP;
        if !item.target.is_empty() && target_x < right {
            p.label_fit(item.target, typography::LABEL, target_x, cy, right - target_x, p.palette().text_tertiary);
        }
    }
}

fn paint_output(p: &mut Painter, text: &str, block: Rect) {
    p.quad(Quad::new(block, metrics::ROW_RADIUS, p.palette().well));
    let color = p.palette().text_secondary;
    for (index, line) in text.lines().take(metrics::STEP_OUTPUT_LINES).enumerate() {
        let cy = block.y + metrics::STEP_OUTPUT_PAD + metrics::STEP_OUTPUT_LINE * (index as f32 + 0.5);
        p.label_fit(line, typography::CODE_SMALL, block.x + OUTPUT_INSET, cy, block.w - OUTPUT_INSET * 2.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(output: Option<&str>) -> StepItem<'_> {
        StepItem { icon: crate::icons::PLAY, verb: "Bash", target: "", failed: false, output }
    }

    #[test]
    fn height_is_one_row_per_step_plus_each_opened_output() {
        assert_eq!(StepList::height(&[]), 0.0);
        assert_eq!(StepList::height(&[item(None), item(None), item(None)]), metrics::STEP_ROW * 3.0);
        let opened = output_height("a\nb");
        assert_eq!(StepList::height(&[item(Some("a\nb"))]), metrics::STEP_ROW + opened);
    }

    #[test]
    fn a_long_output_is_capped_in_lines() {
        let long = (0..100).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(
            output_height(&long),
            metrics::STEP_OUTPUT_LINES as f32 * metrics::STEP_OUTPUT_LINE + metrics::STEP_OUTPUT_PAD * 2.0
        );
    }
}
