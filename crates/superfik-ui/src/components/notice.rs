//! A notice pinned above the canvas: a raised pill with a danger dot, the
//! message and a close button. It never decides what closing means.

use superfik_render::{Point, Quad, Rect};

use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy)]
pub struct Notice<'a> {
    pub text: &'a str,
    pub width: f32,
    pub reveal: f32,
    pub close_hovered: bool,
    pub informative: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoticeHits {
    pub rect: Rect,
    pub close: Rect,
}

const DOT: f32 = 7.0;
const CLOSE: f32 = 22.0;

impl Notice<'_> {
    fn text_width(&self) -> f32 {
        self.width - metrics::NOTICE_PADDING * 3.0 - DOT - CLOSE
    }

    pub fn height(&self, p: &mut Painter) -> f32 {
        let layout = p.layout(self.text, typography::LABEL, Some(self.text_width()));
        layout.height + metrics::NOTICE_PADDING * 2.0
    }

    pub fn paint(&self, p: &mut Painter, origin: Point) -> NoticeHits {
        let pad = metrics::NOTICE_PADDING;
        let layout = p.layout(self.text, typography::LABEL, Some(self.text_width()));
        let rect = Rect::new(origin.x, origin.y, self.width, layout.height + pad * 2.0);
        p.faded(self.reveal, |p| {
            p.surface_rect(rect, metrics::NODE_RADIUS, metrics::GOO);
            let first_line = rect.y + pad + typography::LABEL.line_height * 0.5;
            let dot = Rect::new(rect.x + pad, first_line - DOT * 0.5, DOT, DOT);
            let dot_color = if self.informative { p.palette().form_accent } else { p.palette().danger };
            p.quad(Quad::new(dot, DOT * 0.5, dot_color));
            p.text(&layout, Point::new(dot.right() + pad, rect.y + pad), p.palette().text_body);
        });
        let close = Rect::new(
            rect.right() - pad * 0.5 - CLOSE,
            rect.y + (pad - (CLOSE - typography::LABEL.line_height) * 0.5),
            CLOSE,
            CLOSE,
        );
        p.faded(self.reveal, |p| {
            if self.close_hovered {
                p.quad(Quad::new(close, metrics::ROW_RADIUS, p.palette().hover));
            }
            p.icon_at(icons::CLOSE, close.center(), metrics::ICON_SMALL, Some(p.palette().icon_muted));
        });
        NoticeHits { rect, close }
    }
}
