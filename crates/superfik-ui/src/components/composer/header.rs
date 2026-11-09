//! Composer header: model label (with morph) and effort.

use superfik_motion::Easing;
use superfik_render::{Point, Rect};

use super::{ComposerPart, ComposerProps};
use crate::components::meter::EffortMeter;
use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, motion, typography};

const EFFORT_RIGHT: f32 = 427.0;

pub(super) fn paint(p: &mut Painter, props: &ComposerProps) -> (Rect, Rect, Rect) {
    let o = props.origin;
    let cy = o.y + 23.0;
    let x = o.x + metrics::WELL_INSET;
    p.icon_at(icons::BURST, Point::new(x + metrics::BURST * 0.5, cy), metrics::BURST, Some(p.palette().model));
    let model = paint_model_label(p, props, x + metrics::BURST + 5.0, cy);

    let effort = EffortMeter {
        label: props.effort,
        level: props.effort_level,
        ultra: props.ultra,
        hovered: props.hovered == Some(ComposerPart::Effort),
        time: props.time,
    }
    .paint(p, o.x + EFFORT_RIGHT, cy);
    let track = EffortMeter::track(o.x + EFFORT_RIGHT, cy);
    (Rect::new(o.x + metrics::WELL_INSET - 4.0, o.y + 8.0, model + metrics::BURST + 14.0, 30.0), effort, track)
}

fn paint_model_label(p: &mut Painter, props: &ComposerProps, x: f32, cy: f32) -> f32 {
    let current = p.layout(props.model, typography::MODEL, None);
    let Some((previous, t)) = props.previous_model.filter(|(_, t)| *t < 1.0) else {
        let color = if props.hovered == Some(ComposerPart::Model) { p.palette().text_hover } else { p.palette().text };
        p.text(&current, Point::new(x, (cy - current.height * 0.5).round()), color);
        return current.width;
    };
    let k = Easing::IN_OUT.apply(t);
    let text = p.palette().text;
    let old = p.layout(previous, typography::MODEL, None);
    let y = (cy - current.height * 0.5).round();
    p.text_with(&old, Point::new(x, y - 4.0 * k), |_| (text.faded(1.0 - k), motion::BLUR * k));
    p.text_with(&current, Point::new(x, y + 4.0 * (1.0 - k)), |_| (text.faded(k), motion::BLUR * (1.0 - k)));
    old.width + (current.width - old.width) * k
}
