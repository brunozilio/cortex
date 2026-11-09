//! The Dock tile.
//!
//! [`Tile`] is the app-facing seam: hand it an [`Image`] and it becomes the
//! application icon until the next one. [`MacDock`] draws it with AppKit;
//! [`Plain`] leaves the bundled icon alone (other platforms, or the feature
//! switched off); [`Recorder`] keeps what it was given, for tests.

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "macos")]
pub use mac::MacDock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        (rgba.len() == width as usize * height as usize * 4).then_some(Self { width, height, rgba })
    }

    pub fn premultiplied(&self) -> Vec<u8> {
        self.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|px| {
                let a = u16::from(px[3]);
                let mul = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
                [mul(px[0]), mul(px[1]), mul(px[2]), px[3]]
            })
            .collect()
    }
}

pub trait Tile {
    fn set(&mut self, image: &Image);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Plain;

impl Tile for Plain {
    fn set(&mut self, _image: &Image) {}
}

#[derive(Debug, Default)]
pub struct Recorder {
    pub images: Vec<Image>,
}

impl Tile for Recorder {
    fn set(&mut self, image: &Image) {
        self.images.push(image.clone());
    }
}

pub fn system() -> Box<dyn Tile> {
    #[cfg(target_os = "macos")]
    if let Some(dock) = MacDock::new() {
        return Box::new(dock);
    }
    Box::new(Plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_checks_its_size() {
        assert!(Image::new(2, 1, vec![0; 8]).is_some());
        assert!(Image::new(2, 1, vec![0; 7]).is_none());
    }

    #[test]
    fn premultiplies_colour_by_alpha() {
        let image = Image::new(1, 1, vec![255, 128, 0, 128]).expect("size");
        assert_eq!(image.premultiplied(), vec![128, 64, 0, 128]);
    }

    #[test]
    fn recorder_keeps_images() {
        let mut dock = Recorder::default();
        dock.set(&Image::new(1, 1, vec![1, 2, 3, 4]).expect("size"));
        assert_eq!(dock.images.len(), 1);
    }
}
