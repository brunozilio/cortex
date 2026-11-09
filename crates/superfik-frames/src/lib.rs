//! Browser pages streamed from the server (ADR 0010): a page's picture
//! travels as the rectangles that changed since its last frame, each
//! encoded on its own, and is painted back on the client.
//!
//! - [`damage`] finds what changed between two frames, by tiles.
//! - [`PageEncoder`] is the seam a codec fills; [`JpegTiles`] encodes each
//!   changed rectangle as a JPEG. A video codec (H.264 through
//!   VideoToolbox) would be another implementation.
//! - [`decode`] turns a tile back into pixels for the client.
//!
//! Pure: no IO, no GPU.

mod damage;
mod decode;
mod jpeg;

pub use damage::{Rect, damage};
pub use decode::{Decoded, decode};
pub use jpeg::JpegTiles;
use superfik_core::BrowserId;
use superfik_core::page::PageFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Rgba,
    Bgra,
}

#[derive(Debug, Clone, Copy)]
pub struct Pixels<'a> {
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub order: Order,
}

impl Pixels<'_> {
    fn row(&self, y: u32) -> &[u8] {
        let start = y as usize * self.stride;
        &self.data[start..start + self.width as usize * 4]
    }

    pub fn is_whole(&self) -> bool {
        self.stride >= self.width as usize * 4 && self.data.len() >= self.stride * self.height as usize
    }
}

pub trait PageEncoder: Send {
    fn encode(&mut self, id: BrowserId, pixels: &Pixels<'_>, whole: bool) -> Option<PageFrame>;
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("the tile could not be decoded: {0}")]
    Decode(String),
}
