//! A row of labels where one is selected (Stacked | Split).

use superfik_render::{Point, Quad, Rect};

use crate::painter::Painter;
use crate::theme::colors::DISABLED_OPACITY;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy)]
pub struct Segmented<'a> {
    pub labels: &'a [&'a str],
    pub selected: usize,
    pub hovered: Option<usize>,
}

impl Segmented<'_> {
    pub const HEIGHT: f32 = 28.0;

    fn widths(&self, p: &mut Painter) -> Vec<f32> {
        self.labels.iter().map(|l| p.layout(l, typography::CHIP, None).width + 22.0).collect()
    }

    pub fn width(&self, p: &mut Painter) -> f32 {
        self.widths(p).iter().sum::<f32>() + 4.0
    }

    pub fn paint_disabled(&self, p: &mut Painter, origin: Point) {
        let idle = Segmented { hovered: None, ..*self };
        p.faded(DISABLED_OPACITY, |p| idle.paint(p, origin));
    }

    pub fn paint(&self, p: &mut Painter, origin: Point) -> Vec<Rect> {
        let widths = self.widths(p);
        let total: f32 = widths.iter().sum::<f32>() + 4.0;
        let shell = Rect::new(origin.x, origin.y, total, Self::HEIGHT);
        p.quad(Quad::new(shell, metrics::CHIP_RADIUS, p.palette().control));
        let mut x = shell.x + 2.0;
        let mut rects = Vec::with_capacity(self.labels.len());
        for (i, (label, width)) in self.labels.iter().zip(widths).enumerate() {
            let rect = Rect::new(x, shell.y + 2.0, width, Self::HEIGHT - 4.0);
            let selected = i == self.selected;
            if selected || self.hovered == Some(i) {
                let fill = if selected { p.palette().control_active } else { p.palette().hover };
                p.quad(Quad::new(rect, metrics::CHIP_RADIUS - 1.0, fill));
            }
            let color = if selected { p.palette().text } else { p.palette().text_secondary };
            p.label(label, typography::CHIP, rect.x + 11.0, rect.center().y, color);
            rects.push(rect);
            x += width;
        }
        rects
    }
}
