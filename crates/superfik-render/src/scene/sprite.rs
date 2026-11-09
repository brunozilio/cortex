//! Textured primitives: images, icons and text glyphs.

use cosmic_text::CacheKey;

use crate::color::Color;
use crate::geometry::Rect;
use crate::icons::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageSprite {
    pub image: ImageId,
    pub rect: Rect,
    pub radius: f32,
    pub opacity: f32,
    pub blur: f32,
    pub clip: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconSprite {
    pub icon: Icon,
    pub rect: Rect,
    pub tint: Option<Color>,
    pub opacity: f32,
    pub clip: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphSprite {
    pub key: CacheKey,
    pub x: i32,
    pub y: i32,
    pub color: Color,
    pub blur: f32,
    pub clip: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PictureSprite {
    pub picture: crate::pictures::Picture,
    pub rect: Rect,
    pub radius: f32,
    pub opacity: f32,
    pub clip: Rect,
}
