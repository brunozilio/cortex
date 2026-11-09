//! The frame shared by every canvas node: surface, header, optional ear.

use superfik_render::{Color, GooShape, Icon, Point, Rect};

use super::badge::Badge;
use super::knob::Knob;
use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct NodeFrame<'a> {
    pub rect: Rect,
    pub icon: Icon,
    pub tint: Option<Color>,
    pub title: &'a str,
    pub icon_size: f32,
    pub count: Option<&'a str>,
    pub busy: bool,
    pub ear: Option<EarSide>,
    pub ear_hovered: bool,
    pub ear_glyph: Icon,
    pub content: f32,
    pub time: f32,
    pub plain: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NodeHits {
    pub header: Rect,
    pub knob: Option<Rect>,
}

impl<'a> NodeFrame<'a> {
    pub fn new(rect: Rect, icon: Icon, tint: Option<Color>, title: &'a str) -> Self {
        Self {
            rect,
            icon,
            tint,
            title,
            icon_size: metrics::HEADER_ICON,
            count: None,
            busy: false,
            ear: None,
            ear_hovered: false,
            ear_glyph: icons::CLOSE,
            content: 1.0,
            time: 0.0,
            plain: false,
        }
    }

    pub fn paint(&self, p: &mut Painter) -> NodeHits {
        let r = self.rect;
        p.surface_rect(r, metrics::NODE_RADIUS, metrics::GOO);
        let knob = self.ear.filter(|_| r.h >= metrics::NODE_HEADER).map(|side| self.paint_ear(p, side));
        let header = Rect::new(r.x, r.y, r.w, metrics::NODE_HEADER);
        let cy = r.y + 21.0;
        if self.plain {
            return NodeHits { header, knob };
        }
        p.faded(self.content, |p| {
            p.icon_at(self.icon, Point::new(r.x + metrics::HEADER_ICON_X, cy), self.icon_size, self.tint);
            p.label(
                self.title,
                typography::HEADER,
                r.x + metrics::HEADER_TITLE_X,
                cy + 1.0,
                p.palette().text_secondary,
            );
            let mut right = r.right() - 13.0;
            if let Some(text) = self.count {
                right = Badge { text }.paint(p, right, cy).x - 6.0;
            }
            if self.busy {
                busy_dots(p, right - 9.0, cy, self.time);
            }
        });
        NodeHits { header, knob }
    }

    pub fn ear_rect(rect: Rect, side: EarSide) -> Rect {
        let cy = rect.y + 40.0;
        match side {
            EarSide::Right => Rect::new(rect.right() - 10.0, cy - 15.0, 39.0, 30.0),
            EarSide::Left => Rect::new(rect.x - 29.0, cy - 15.0, 39.0, 30.0),
        }
    }

    pub const EAR_RADIUS: f32 = 11.0;

    fn paint_ear(&self, p: &mut Painter, side: EarSide) -> Rect {
        let r = self.rect;
        let ear = Self::ear_rect(r, side);
        let cy = ear.y + ear.h * 0.5;
        let center = match side {
            EarSide::Right => Point::new(r.right() + 14.0, cy),
            EarSide::Left => Point::new(r.x - 14.0, cy),
        };
        p.surface(
            superfik_render::GooPrim::new(
                GooShape::RoundRect { rect: ear, radius: Self::EAR_RADIUS },
                p.palette().surface_top,
                metrics::GOO,
            )
            .gradient(p.palette().surface_top, p.palette().surface_bottom),
        );
        p.faded(self.content, |p| Knob { center, hovered: self.ear_hovered, glyph: self.ear_glyph }.paint(p))
    }
}

pub fn busy_dots(p: &mut Painter, center_x: f32, cy: f32, time: f32) {
    for i in 0..3 {
        let phase = (time * 5.0 - i as f32 * 0.7).sin() * 0.5 + 0.5;
        let color = p.palette().text_secondary.faded(0.35 + 0.65 * phase);
        let c = Point::new(center_x - 5.0 + i as f32 * 5.0, cy);
        p.quad(superfik_render::Quad::new(Rect::new(c.x - 1.4, c.y - 1.4, 2.8, 2.8), 1.4, color));
    }
}
