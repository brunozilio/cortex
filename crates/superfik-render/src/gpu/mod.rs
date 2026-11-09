//! wgpu plumbing: device/surface setup, pipelines and GPU buffers.

mod buffer;
mod context;
mod globals;
mod goo;
mod quad;
mod sprite;
mod textures;

pub use context::{GpuContext, GpuError, OpenDevice};
pub(crate) use globals::{Globals, globals_layout};
pub(crate) use goo::GooPipeline;
pub(crate) use quad::QuadPipeline;
pub(crate) use sprite::{SpriteInstance, SpriteKind, SpritePipeline};
pub(crate) use textures::{AtlasTexture, ImageTextures, texture_layout};
