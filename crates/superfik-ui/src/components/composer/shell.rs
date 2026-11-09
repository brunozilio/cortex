//! The composer's surfaces: an L-shaped body that leaves a notch for the
//! send button, and an input well notched around the mic button.

use superfik_render::{Flow, GooPrim, GooShape, Point, Rect};

use crate::painter::Painter;
use crate::theme::{metrics, typography};

pub(super) fn paint_body(p: &mut Painter, o: Point) {
    let shape = GooShape::Notched {
        rect: Rect::new(o.x, o.y, 440.0, 190.0),
        radius: metrics::NODE_RADIUS,
        notch: Rect::new(o.x + 401.0, o.y + 148.0, 80.0, 80.0),
        notch_radius: metrics::NODE_RADIUS,
    };
    p.surface(
        GooPrim::new(shape, p.palette().surface_top, metrics::GOO)
            .gradient(p.palette().surface_top, p.palette().surface_bottom),
    );
}

pub(super) fn paint_input_well(p: &mut Painter, o: Point) -> (Rect, Flow) {
    let radius = metrics::WELL_RADIUS;
    let shape = GooShape::Notched {
        rect: Rect::new(o.x + metrics::WELL_INSET, o.y + 46.0, 420.0, 95.0),
        radius,
        notch: Rect::new(o.x - 40.0, o.y + 105.0, 87.0, 80.0),
        notch_radius: radius,
    };
    p.well(shape, p.palette().well, 6.0);
    let area = Rect::new(o.x + 24.0, o.y + 52.0, 394.0, metrics::INPUT_LINES * typography::INPUT.line_height);
    (area, Flow { width: area.w, inset_after: metrics::INPUT_FULL_LINES, inset: metrics::INPUT_INSET })
}
