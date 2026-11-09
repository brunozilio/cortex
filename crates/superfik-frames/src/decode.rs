//! A tile back into pixels.

use superfik_core::page::{FrameTile, TileCodec};

use crate::FrameError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

pub fn decode(tile: &FrameTile) -> Result<Decoded, FrameError> {
    let format = match tile.codec {
        TileCodec::Jpeg => image::ImageFormat::Jpeg,
    };
    let image = image::load_from_memory_with_format(&tile.data, format)
        .map_err(|error| FrameError::Decode(error.to_string()))?
        .to_rgba8();
    if image.dimensions() != (tile.w, tile.h) {
        return Err(FrameError::Decode(format!(
            "{}×{} picture for a {}×{} tile",
            image.width(),
            image.height(),
            tile.w,
            tile.h
        )));
    }
    Ok(Decoded { w: tile.w, h: tile.h, rgba: image.into_raw() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_and_misfit_tiles_are_refused() {
        let garbage = FrameTile { x: 0, y: 0, w: 2, h: 2, codec: TileCodec::Jpeg, data: vec![1, 2, 3] };
        assert!(decode(&garbage).is_err());
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode(&[0u8; 4 * 4 * 3], 4, 4, image::ExtendedColorType::Rgb8)
            .expect("encodes");
        let misfit = FrameTile { x: 0, y: 0, w: 8, h: 8, codec: TileCodec::Jpeg, data: jpeg };
        assert!(decode(&misfit).is_err());
    }
}
