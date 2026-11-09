//! The scene: what to draw this frame, in logical pixels.
//!
//! A scene is an ordered list of [`Layer`]s. Inside a layer the draw order
//! is fixed: goo shadows, goo surfaces, quads, images, icons, glyphs. Use a
//! new layer whenever something must cover text drawn earlier (popovers).

pub(crate) mod goo;
mod path;
mod quad;
mod sprite;

pub use goo::{GooPrim, GooShape, GooStyle};
pub use path::StepPath;
pub use quad::{Fill, Pattern, Quad, Shadow};
pub use sprite::{GlyphSprite, IconSprite, ImageId, ImageSprite, PictureSprite};

use crate::color::Color;

#[derive(Debug, Default, Clone)]
pub struct Layer {
    pub goo: Vec<GooPrim>,
    pub goo_style: GooStyle,
    pub quads: Vec<Quad>,
    pub images: Vec<ImageSprite>,
    pub pictures: Vec<PictureSprite>,
    pub icons: Vec<IconSprite>,
    pub glyphs: Vec<GlyphSprite>,
}

impl Layer {
    pub fn is_empty(&self) -> bool {
        self.goo.is_empty()
            && self.quads.is_empty()
            && self.images.is_empty()
            && self.pictures.is_empty()
            && self.icons.is_empty()
            && self.glyphs.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatternTones {
    pub meter_lit: Color,
    pub meter_checker: [Color; 2],
    pub ultra_off: Color,
    pub sparkle_ink: Color,
    pub shimmer: f32,
}

impl Default for PatternTones {
    fn default() -> Self {
        Self {
            meter_lit: Color::hex(0xdbdbdb),
            meter_checker: [Color::hex(0x454545), Color::hex(0x5c5c5c)],
            ultra_off: Color::hex(0x2b2b2b),
            sparkle_ink: Color::hex(0x292929),
            shimmer: 0.035,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub background: Color,
    pub layers: Vec<Layer>,
    pub pattern_tones: PatternTones,
}

impl Scene {
    pub fn new(background: Color, layer_count: usize) -> Self {
        Self { background, layers: vec![Layer::default(); layer_count.max(1)], pattern_tones: PatternTones::default() }
    }

    pub fn layer(&mut self, index: usize) -> &mut Layer {
        if index >= self.layers.len() {
            self.layers.resize_with(index + 1, Layer::default);
        }
        &mut self.layers[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_access_grows_stack() {
        let mut scene = Scene::new(Color::BLACK, 1);
        assert!(scene.layer(0).is_empty());
        scene.layer(3);
        assert_eq!(scene.layers.len(), 4);
    }
}
