//! Thick goo connectors between nodes, with particles flowing while busy.

use superfik_render::{GooPrim, GooShape, Point, Quad, Rect, StepPath};

use crate::painter::Painter;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy)]
pub struct Flow {
    pub time: f32,
    pub intensity: f32,
}

#[derive(Debug, Clone)]
pub struct Connector {
    pub path: StepPath,
    pub reach: f32,
    pub flow: Option<Flow>,
}

impl Connector {
    pub fn paint(&self, p: &mut Painter) {
        let path = truncate(&self.path, self.reach);
        for shape in path.shapes(metrics::CONNECTOR_RADIUS, metrics::CONNECTOR_CORNER) {
            p.surface(GooPrim::new(shape, p.palette().connector, metrics::CONNECTOR_FILLET).line());
        }
        if self.reach > 0.0
            && self.reach < 1.0
            && let Some(&tip) = path.points().last()
        {
            let radius = metrics::CONNECTOR_TIP * (self.reach * 4.0).min(1.0);
            let drop = GooShape::Circle { center: tip, radius };
            p.surface(GooPrim::new(drop, p.palette().connector, metrics::CONNECTOR_FILLET).line());
        }
        if let Some(flow) = self.flow.filter(|f| f.intensity > 0.0) {
            paint_flow(p, &path, flow);
        }
    }
}

fn length(points: &[Point]) -> f32 {
    points.windows(2).map(|w| w[0].distance(w[1])).sum()
}

pub fn point_along(points: &[Point], mut d: f32) -> Point {
    for w in points.windows(2) {
        let seg = w[0].distance(w[1]);
        if d <= seg && seg > 0.0 {
            let k = d / seg;
            return Point::new(w[0].x + (w[1].x - w[0].x) * k, w[0].y + (w[1].y - w[0].y) * k);
        }
        d -= seg;
    }
    points.last().copied().unwrap_or_default()
}

fn truncate(path: &StepPath, fraction: f32) -> StepPath {
    if fraction >= 1.0 {
        return path.clone();
    }
    let points = path.points();
    let target = length(points) * fraction.max(0.0);
    let mut out = vec![points[0]];
    let mut walked = 0.0;
    for w in points.windows(2) {
        let seg = w[0].distance(w[1]);
        if walked + seg >= target {
            out.push(point_along(&[w[0], w[1]], target - walked));
            break;
        }
        walked += seg;
        out.push(w[1]);
    }
    StepPath::new(out)
}

fn paint_flow(p: &mut Painter, path: &StepPath, flow: Flow) {
    let points = path.points();
    let total = length(points);
    let spacing = 7.0;
    let offset = (flow.time * 38.0) % spacing;
    let count = (total / spacing) as usize;
    let particles = p.palette().particles;
    for i in 0..count {
        let d = i as f32 * spacing + offset;
        let c = point_along(points, d);
        let seed = (i * 7 + (flow.time * 3.0) as usize) % particles.len();
        let twinkle = ((flow.time * 6.0 + i as f32 * 1.7).sin() * 0.5 + 0.5) * 0.6 + 0.4;
        let color = particles[seed].faded(twinkle * flow.intensity);
        p.quad(Quad::new(Rect::new(c.x - 1.1, c.y - 1.1, 2.2, 2.2), 1.1, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_along_polyline() {
        let pts = [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)];
        assert_eq!(point_along(&pts, 5.0), Point::new(5.0, 0.0));
        assert_eq!(point_along(&pts, 15.0), Point::new(10.0, 5.0));
        assert_eq!(point_along(&pts, 99.0), Point::new(10.0, 10.0));
    }

    #[test]
    fn truncates_by_length() {
        let path = StepPath::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)]);
        let half = truncate(&path, 0.5);
        assert_eq!(half.points().last().copied(), Some(Point::new(10.0, 0.0)));
        assert_eq!(truncate(&path, 1.0), path);
    }
}
