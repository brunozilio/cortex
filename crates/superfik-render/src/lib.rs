//! GPU renderer for Superfik.
//!
//! The renderer knows nothing about the product. It draws a [`Scene`]: an
//! ordered list of layers holding goo surfaces (SDF shapes fused with a
//! smooth union), rounded quads, text and sprites. Everything is expressed
//! in logical pixels; the renderer converts to device pixels.

pub mod atlas;
pub mod color;
pub mod geometry;
pub mod gpu;
pub mod icons;
pub mod pictures;
pub mod scene;
pub mod text;

mod renderer;

pub use color::Color;
pub use geometry::{Point, Rect};
pub use icons::Icon;
pub use pictures::Picture;
pub use renderer::{RenderError, Renderer};
pub use scene::{
    Fill, GlyphSprite, GooPrim, GooShape, GooStyle, IconSprite, ImageId, ImageSprite, Layer, Pattern, PatternTones,
    PictureSprite, Quad, Scene, Shadow, StepPath,
};
pub use text::{
    FallbackFaces, Flow, FontFamily, FontWeight, NoFaces, Script, StyleRun, SystemFaces, TextLayout, TextStyle,
    TextSystem,
};
