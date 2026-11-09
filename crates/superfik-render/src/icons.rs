//! Vector icons rasterized on demand into the colour atlas.

use resvg::{tiny_skia, usvg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon {
    pub name: &'static str,
    pub svg: &'static str,
}

impl Icon {
    pub const fn new(name: &'static str, svg: &'static str) -> Self {
        Self { name, svg }
    }
}

pub fn rasterize(icon: &Icon, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(icon.svg, &usvg::Options::default())
        .map_err(|error| tracing::warn!(icon = icon.name, %error, "invalid icon svg"))
        .ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    let size = tree.size();
    let transform = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(pixmap.take_demultiplied())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOT: Icon = Icon::new(
        "dot",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><rect width="4" height="4" fill="#ff0000"/></svg>"##,
    );

    #[test]
    fn rasterizes_to_requested_size() {
        let pixels = rasterize(&DOT, 8, 8).expect("raster");
        assert_eq!(pixels.len(), 8 * 8 * 4);
        assert_eq!(&pixels[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn invalid_svg_is_none() {
        assert!(rasterize(&Icon::new("bad", "<nope"), 4, 4).is_none());
    }
}
