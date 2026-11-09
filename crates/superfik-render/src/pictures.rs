//! Pictures: whole SVG documents (an agent's visuals) rasterized on demand
//! at the size they are drawn, each into its own texture, and released
//! once a few frames pass without them. A visual streaming in is a new
//! picture with every shape, so only a few undrawn ones are kept.

use std::collections::HashMap;
use std::sync::Arc;

use resvg::{tiny_skia, usvg};

use crate::scene::ImageId;
use crate::text::system::{BUNDLED_MONO, FAMILY, FONTS};

const MAX_SIDE: u32 = 4096;
const KEEP_FRAMES: u64 = 120;
const SPARE: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub key: u64,
    pub svg: Arc<str>,
}

pub fn faces() -> Arc<usvg::fontdb::Database> {
    let mut faces = usvg::fontdb::Database::new();
    for font in FONTS {
        faces.load_font_data(font.to_vec());
    }
    faces.load_font_data(BUNDLED_MONO.to_vec());
    faces.set_sans_serif_family(FAMILY);
    Arc::new(faces)
}

pub fn rasterize(svg: &str, faces: &Arc<usvg::fontdb::Database>, w: u32, h: u32) -> Option<Vec<u8>> {
    let options = usvg::Options { font_family: FAMILY.into(), fontdb: faces.clone(), ..usvg::Options::default() };
    let tree =
        usvg::Tree::from_str(svg, &options).map_err(|error| tracing::warn!(%error, "invalid picture svg")).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    let size = tree.size();
    let transform = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(pixmap.take_demultiplied())
}

pub fn device_size(w: f32, h: f32, scale: f32) -> (u32, u32) {
    let (w, h) = ((w * scale).round().max(1.0), (h * scale).round().max(1.0));
    let shrink = (MAX_SIDE as f32 / w.max(h)).min(1.0);
    ((w * shrink).round() as u32, (h * shrink).round() as u32)
}

struct Entry {
    image: Option<ImageId>,
    last_seen: u64,
}

pub(crate) struct Pictures {
    faces: Option<Arc<usvg::fontdb::Database>>,
    entries: HashMap<(u64, u32, u32), Entry>,
    frame: u64,
}

impl Pictures {
    pub fn new() -> Self {
        Self { faces: None, entries: HashMap::new(), frame: 0 }
    }

    pub fn image(
        &mut self,
        picture: &Picture,
        (w, h): (u32, u32),
        upload: impl FnOnce(&[u8], u32, u32) -> ImageId,
    ) -> Option<ImageId> {
        let frame = self.frame;
        if let Some(entry) = self.entries.get_mut(&(picture.key, w, h)) {
            entry.last_seen = frame;
            return entry.image;
        }
        let faces = self.faces.get_or_insert_with(faces);
        let image = rasterize(&picture.svg, faces, w, h).map(|rgba| upload(&rgba, w, h));
        self.entries.insert((picture.key, w, h), Entry { image, last_seen: frame });
        image
    }

    pub fn sweep(&mut self) -> Vec<ImageId> {
        let drawn = self.frame;
        self.frame += 1;
        let mut undrawn: Vec<((u64, u32, u32), u64)> =
            self.entries.iter().filter(|(_, e)| e.last_seen < drawn).map(|(key, e)| (*key, e.last_seen)).collect();
        undrawn.sort_by_key(|(_, seen)| std::cmp::Reverse(*seen));
        let mut stale = Vec::new();
        for (i, (key, seen)) in undrawn.into_iter().enumerate() {
            if i >= SPARE || self.frame - seen > KEEP_FRAMES {
                stale.extend(self.entries.remove(&key).and_then(|entry| entry.image));
            }
        }
        stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 20"><rect width="40" height="20" fill="#ffffff"/><text x="2" y="16" font-size="16" fill="#000000">Hi</text></svg>"##;

    #[test]
    fn text_is_set_in_the_bundled_faces() {
        let pixels = rasterize(TEXT, &faces(), 40, 20).expect("raster");
        assert_eq!(pixels.len(), 40 * 20 * 4);
        assert!(pixels.chunks(4).any(|px| px[0] < 128), "the text left ink");
    }

    #[test]
    fn sizes_are_capped_keeping_the_shape() {
        assert_eq!(device_size(440.0, 200.0, 2.0), (880, 400));
        assert_eq!(device_size(440.0, 8800.0, 2.0), (205, 4096));
    }

    #[test]
    fn pictures_are_rasterized_once_and_released_when_unused() {
        let mut pictures = Pictures::new();
        let picture = Picture { key: 1, svg: TEXT.into() };
        let mut uploads = 0;
        for _ in 0..3 {
            let image = pictures.image(&picture, (40, 20), |_, _, _| {
                uploads += 1;
                ImageId(7)
            });
            assert_eq!(image, Some(ImageId(7)));
            assert!(pictures.sweep().is_empty());
        }
        assert_eq!(uploads, 1);
        let released: Vec<ImageId> = (0..=KEEP_FRAMES).flat_map(|_| pictures.sweep()).collect();
        assert_eq!(released, vec![ImageId(7)]);
    }

    #[test]
    fn a_streaming_picture_keeps_only_a_few_earlier_versions() {
        let mut pictures = Pictures::new();
        for id in 0..20u32 {
            let key = u64::from(id);
            pictures.image(&Picture { key, svg: TEXT.into() }, (40, 20), |_, _, _| ImageId(id));
            pictures.sweep();
        }
        assert_eq!(pictures.entries.len(), SPARE + 1);
        assert!(pictures.entries.contains_key(&(19, 40, 20)), "the latest stays");
    }

    #[test]
    fn a_broken_picture_is_not_tried_again() {
        let mut pictures = Pictures::new();
        let broken = Picture { key: 2, svg: "<svg".into() };
        assert_eq!(pictures.image(&broken, (4, 4), |_, _, _| ImageId(1)), None);
        assert_eq!(pictures.image(&broken, (4, 4), |_, _, _| unreachable!("parsed again")), None);
    }
}
