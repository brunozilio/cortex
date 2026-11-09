//! Goo primitives: SDF shapes fused with a smooth union.

use crate::color::Color;
use crate::geometry::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GooShape {
    RoundRect { rect: Rect, radius: f32 },
    Capsule { a: Point, b: Point, radius: f32 },
    Arc { center: Point, radius: f32, start: f32, sweep: f32, thickness: f32 },
    Circle { center: Point, radius: f32 },
    Notched { rect: Rect, radius: f32, notch: Rect, notch_radius: f32 },
}

impl GooShape {
    pub fn translate(self, dx: f32, dy: f32) -> Self {
        let mv = |p: Point| Point::new(p.x + dx, p.y + dy);
        match self {
            Self::RoundRect { rect, radius } => Self::RoundRect { rect: rect.translate(dx, dy), radius },
            Self::Capsule { a, b, radius } => Self::Capsule { a: mv(a), b: mv(b), radius },
            Self::Arc { center, radius, start, sweep, thickness } => {
                Self::Arc { center: mv(center), radius, start, sweep, thickness }
            }
            Self::Circle { center, radius } => Self::Circle { center: mv(center), radius },
            Self::Notched { rect, radius, notch, notch_radius } => {
                Self::Notched { rect: rect.translate(dx, dy), radius, notch: notch.translate(dx, dy), notch_radius }
            }
        }
    }

    pub fn bounds(&self) -> Rect {
        match *self {
            Self::RoundRect { rect, .. } => rect,
            Self::Capsule { a, b, radius } => Rect::from_points(a, b).translate(-radius, -radius).grow(radius * 2.0),
            Self::Arc { center, radius, thickness, .. } => {
                let r = radius + thickness;
                Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0)
            }
            Self::Circle { center, radius } => {
                Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0)
            }
            Self::Notched { rect, .. } => rect,
        }
    }
}

trait Grow {
    fn grow(self, by: f32) -> Self;
}

impl Grow for Rect {
    fn grow(self, by: f32) -> Self {
        Rect::new(self.x, self.y, self.w + by, self.h + by)
    }
}

const MIN_REACH: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GooPrim {
    pub shape: GooShape,
    pub top: Color,
    pub bottom: Color,
    pub smooth: f32,
    pub opacity: f32,
    pub ghost: bool,
    pub line: bool,
}

impl GooPrim {
    pub fn new(shape: GooShape, color: Color, smooth: f32) -> Self {
        Self { shape, top: color, bottom: color, smooth, opacity: 1.0, ghost: false, line: false }
    }

    pub fn gradient(mut self, top: Color, bottom: Color) -> Self {
        self.top = top;
        self.bottom = bottom;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.ghost = true;
        self
    }

    pub fn line(mut self) -> Self {
        self.line = true;
        self
    }

    pub fn reach(&self) -> Rect {
        let b = self.shape.bounds();
        let pad = self.smooth.max(MIN_REACH);
        Rect::new(b.x - pad, b.y - pad, b.w + pad * 2.0, b.h + pad * 2.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GooStyle {
    pub highlight: Color,
    pub shadow: Color,
    pub shadow_offset: f32,
    pub shadow_blur: f32,
}

impl Default for GooStyle {
    fn default() -> Self {
        Self {
            highlight: Color::WHITE.with_alpha(0.04),
            shadow: Color::BLACK.with_alpha(0.32),
            shadow_offset: 1.5,
            shadow_blur: 3.0,
        }
    }
}

pub fn neighbor_lists(prims: &[GooPrim]) -> Vec<Vec<u32>> {
    let reaches: Vec<Rect> = prims.iter().map(GooPrim::reach).collect();
    reaches
        .iter()
        .map(|a| reaches.iter().enumerate().filter(|(_, b)| a.overlaps(b)).map(|(j, _)| j as u32).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32) -> GooPrim {
        GooPrim::new(GooShape::RoundRect { rect: Rect::new(x, 0.0, 10.0, 10.0), radius: 2.0 }, Color::WHITE, 4.0)
    }

    #[test]
    fn translation_moves_bounds() {
        let shape = GooShape::Circle { center: Point::new(1.0, 2.0), radius: 3.0 };
        assert_eq!(shape.translate(10.0, 20.0).bounds(), shape.bounds().translate(10.0, 20.0));
    }

    #[test]
    fn capsule_bounds_include_radius() {
        let capsule = GooShape::Capsule { a: Point::new(0.0, 0.0), b: Point::new(10.0, 0.0), radius: 3.0 };
        assert_eq!(capsule.bounds(), Rect::new(-3.0, -3.0, 16.0, 6.0));
    }

    #[test]
    fn neighbors_are_symmetric_sorted_and_include_self() {
        let prims = [rect(0.0), rect(15.0), rect(100.0)];
        let lists = neighbor_lists(&prims);
        assert_eq!(lists[0], vec![0, 1]);
        assert_eq!(lists[1], vec![0, 1]);
        assert_eq!(lists[2], vec![2]);
    }

    #[test]
    fn reach_pads_by_smoothing() {
        assert_eq!(rect(0.0).reach(), Rect::new(-4.0, -4.0, 18.0, 18.0));
    }
}
