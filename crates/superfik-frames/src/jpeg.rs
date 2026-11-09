//! Each changed rectangle as a JPEG.

use image::ExtendedColorType;
use image::codecs::jpeg::JpegEncoder;
use superfik_core::BrowserId;
use superfik_core::page::{FrameTile, PageFrame, TileCodec};

use crate::{Order, PageEncoder, Pixels, Rect, damage};

const TILE: u32 = 64;
const WHOLE_SHARE: f64 = 0.6;

#[derive(Debug)]
pub struct JpegTiles {
    quality: u8,
    last: Option<(u32, u32, Vec<u8>)>,
}

impl JpegTiles {
    pub fn new(quality: u8) -> Self {
        Self { quality: quality.clamp(1, 100), last: None }
    }
}

impl Default for JpegTiles {
    fn default() -> Self {
        Self::new(85)
    }
}

impl PageEncoder for JpegTiles {
    fn encode(&mut self, id: BrowserId, pixels: &Pixels<'_>, whole: bool) -> Option<PageFrame> {
        if !pixels.is_whole() || pixels.width == 0 || pixels.height == 0 {
            return None;
        }
        let full = Rect { x: 0, y: 0, w: pixels.width, h: pixels.height };
        let rects = match &self.last {
            Some((w, h, last)) if !whole && (*w, *h) == (pixels.width, pixels.height) => {
                let rects = damage(last, pixels, TILE);
                if rects.is_empty() {
                    return None;
                }
                let changed: u64 = rects.iter().map(Rect::area).sum();
                if changed as f64 > full.area() as f64 * WHOLE_SHARE { vec![full] } else { rects }
            }
            _ => vec![full],
        };
        let tiles: Vec<FrameTile> = rects.iter().filter_map(|rect| self.tile(pixels, *rect)).collect();
        self.last = Some((pixels.width, pixels.height, packed(pixels)));
        Some(PageFrame { id, size: (pixels.width, pixels.height), whole: rects == [full], tiles })
    }
}

impl JpegTiles {
    fn tile(&self, pixels: &Pixels<'_>, rect: Rect) -> Option<FrameTile> {
        let mut rgb = Vec::with_capacity(rect.area() as usize * 3);
        let (r, b) = match pixels.order {
            Order::Rgba => (0, 2),
            Order::Bgra => (2, 0),
        };
        for y in rect.y..rect.y + rect.h {
            let row = &pixels.row(y)[rect.x as usize * 4..(rect.x + rect.w) as usize * 4];
            for px in row.as_chunks::<4>().0 {
                rgb.extend_from_slice(&[px[r], px[1], px[b]]);
            }
        }
        let mut data = Vec::new();
        let encoded = JpegEncoder::new_with_quality(&mut data, self.quality).encode(
            &rgb,
            rect.w,
            rect.h,
            ExtendedColorType::Rgb8,
        );
        encoded.ok()?;
        Some(FrameTile { x: rect.x, y: rect.y, w: rect.w, h: rect.h, codec: TileCodec::Jpeg, data })
    }
}

fn packed(pixels: &Pixels<'_>) -> Vec<u8> {
    (0..pixels.height).flat_map(|y| pixels.row(y).iter().copied()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode;

    fn frame(width: u32, height: u32, colour: [u8; 4]) -> Vec<u8> {
        colour.repeat((width * height) as usize)
    }

    fn pixels(data: &[u8], width: u32, height: u32) -> Pixels<'_> {
        Pixels { data, width, height, stride: width as usize * 4, order: Order::Bgra }
    }

    #[test]
    fn the_first_frame_is_whole_and_an_unchanged_one_sends_nothing() {
        let mut encoder = JpegTiles::default();
        let id = BrowserId::new();
        let data = frame(200, 100, [10, 20, 200, 255]);
        let first = encoder.encode(id, &pixels(&data, 200, 100), false).expect("first frame");
        assert!(first.whole);
        assert_eq!(first.tiles.len(), 1);
        assert!(encoder.encode(id, &pixels(&data, 200, 100), false).is_none());
        assert!(encoder.encode(id, &pixels(&data, 200, 100), true).is_some_and(|f| f.whole), "asked again");
    }

    #[test]
    fn a_small_change_sends_only_its_tile_in_the_right_colours() {
        let mut encoder = JpegTiles::default();
        let id = BrowserId::new();
        let mut data = frame(256, 128, [0, 0, 0, 255]);
        encoder.encode(id, &pixels(&data, 256, 128), false);
        for y in 0..64usize {
            for x in 64..128usize {
                data[(y * 256 + x) * 4] = 255;
            }
        }
        let next = encoder.encode(id, &pixels(&data, 256, 128), false).expect("a change");
        assert!(!next.whole);
        let tile = &next.tiles[..];
        assert_eq!(tile.len(), 1);
        assert_eq!((tile[0].x, tile[0].y, tile[0].w, tile[0].h), (64, 0, 64, 64));
        let decoded = decode(&tile[0]).expect("a JPEG");
        let centre = &decoded.rgba[(32 * 64 + 32) * 4..][..4];
        assert!(centre[2] > 240 && centre[0] < 15, "blue: {centre:?}");
    }

    #[test]
    fn a_new_size_or_a_large_change_is_whole() {
        let mut encoder = JpegTiles::default();
        let id = BrowserId::new();
        encoder.encode(id, &pixels(&frame(64, 64, [0; 4]), 64, 64), false);
        let resized = frame(128, 64, [0; 4]);
        assert!(encoder.encode(id, &pixels(&resized, 128, 64), false).is_some_and(|f| f.whole));
        let repainted = frame(128, 64, [9; 4]);
        assert!(encoder.encode(id, &pixels(&repainted, 128, 64), false).is_some_and(|f| f.whole));
    }
}
