//! A QR code: its modules as squares on a light tile with a quiet zone, so
//! a phone reads it in either theme (dark on light, always).

use superfik_render::{Quad, Rect};

use crate::painter::Painter;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy)]
pub struct QrCode<'a> {
    pub modules: &'a [bool],
    pub width: usize,
}

const QUIET_ZONE: usize = 4;

impl QrCode<'_> {
    pub fn paint(&self, p: &mut Painter, rect: Rect) -> Rect {
        let side = rect.w.min(rect.h);
        let tile = Rect::new(rect.x, rect.y, side, side);
        p.quad(Quad::new(tile, metrics::QR_RADIUS, p.palette().qr_paper));
        if self.width == 0 {
            return tile;
        }
        let span = self.width + QUIET_ZONE * 2;
        let module = side / span as f32;
        let ink = p.palette().qr_ink;
        for (index, dark) in self.modules.iter().enumerate() {
            if !*dark {
                continue;
            }
            let (row, column) = (index / self.width, index % self.width);
            let x = tile.x + (column + QUIET_ZONE) as f32 * module;
            let y = tile.y + (row + QUIET_ZONE) as f32 * module;
            p.quad(Quad::new(Rect::new(x, y, module + 0.25, module + 0.25), 0.0, ink));
        }
        tile
    }
}
