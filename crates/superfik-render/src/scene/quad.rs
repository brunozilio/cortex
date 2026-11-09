//! Rounded rectangles with fills, borders, shadows and procedural patterns.

use crate::color::Color;
use crate::geometry::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    pub top: Color,
    pub bottom: Color,
}

impl Fill {
    pub const fn solid(color: Color) -> Self {
        Self { top: color, bottom: color }
    }

    pub const fn vertical(top: Color, bottom: Color) -> Self {
        Self { top, bottom }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub offset_y: f32,
    pub blur: f32,
}

impl Shadow {
    pub const NONE: Shadow = Shadow { color: Color::TRANSPARENT, offset_y: 0.0, blur: 0.0 };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pattern {
    None,
    Meter { level: f32 },
    Ultra { phase: f32 },
    Sparkle { activity: f32 },
    Shimmer,
    Grain { amount: f32 },
}

impl Pattern {
    pub(crate) fn encode(self) -> (f32, f32) {
        match self {
            Self::None => (0.0, 0.0),
            Self::Meter { level } => (1.0, level),
            Self::Ultra { phase } => (2.0, phase),
            Self::Sparkle { activity } => (3.0, activity),
            Self::Shimmer => (4.0, 0.0),
            Self::Grain { amount } => (5.0, amount),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub rect: Rect,
    pub radius: f32,
    pub fill: Fill,
    pub border: Color,
    pub border_width: f32,
    pub inset: Shadow,
    pub shadow: Shadow,
    pub pattern: Pattern,
    pub opacity: f32,
    pub clip: Rect,
}

impl Quad {
    pub fn new(rect: Rect, radius: f32, color: Color) -> Self {
        Self {
            rect,
            radius,
            fill: Fill::solid(color),
            border: Color::TRANSPARENT,
            border_width: 0.0,
            inset: Shadow::NONE,
            shadow: Shadow::NONE,
            pattern: Pattern::None,
            opacity: 1.0,
            clip: Rect::EVERYTHING,
        }
    }

    pub fn fill(mut self, fill: Fill) -> Self {
        self.fill = fill;
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border = color;
        self.border_width = width;
        self
    }

    pub fn inset(mut self, shadow: Shadow) -> Self {
        self.inset = shadow;
        self
    }

    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = shadow;
        self
    }

    pub fn pattern(mut self, pattern: Pattern) -> Self {
        self.pattern = pattern;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn clip(mut self, clip: Rect) -> Self {
        self.clip = clip;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_encode_kind_and_parameter() {
        assert_eq!(Pattern::None.encode(), (0.0, 0.0));
        assert_eq!(Pattern::Grain { amount: 0.05 }.encode(), (5.0, 0.05));
    }
}
