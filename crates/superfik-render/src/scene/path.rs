//! Orthogonal connector paths with rounded corners, built from goo shapes.

use std::f32::consts::PI;

use super::goo::GooShape;
use crate::geometry::Point;

#[derive(Debug, Clone, PartialEq)]
pub struct StepPath {
    points: Vec<Point>,
}

impl StepPath {
    pub fn new(points: Vec<Point>) -> Self {
        Self { points }
    }

    pub fn horizontal(from: Point, to: Point, turn_x: f32) -> Self {
        if (from.y - to.y).abs() < 0.5 {
            return Self::new(vec![from, to]);
        }
        Self::new(vec![from, Point::new(turn_x, from.y), Point::new(turn_x, to.y), to])
    }

    pub fn vertical(from: Point, to: Point, turn_y: f32) -> Self {
        if (from.x - to.x).abs() < 0.5 {
            return Self::new(vec![from, to]);
        }
        Self::new(vec![from, Point::new(from.x, turn_y), Point::new(to.x, turn_y), to])
    }

    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn shapes(&self, radius: f32, corner: f32) -> Vec<GooShape> {
        let pts = &self.points;
        if pts.len() < 2 {
            return Vec::new();
        }
        let mut shapes = Vec::new();
        let mut cursor = pts[0];
        for i in 1..pts.len() - 1 {
            let (prev, here, next) = (pts[i - 1], pts[i], pts[i + 1]);
            let len_in = prev.distance(here);
            let len_out = here.distance(next);
            let r = corner.min(len_in * 0.5).min(len_out * 0.5);
            let din = direction(prev, here);
            let dout = direction(here, next);
            let a = Point::new(here.x - din.x * r, here.y - din.y * r);
            let b = Point::new(here.x + dout.x * r, here.y + dout.y * r);
            push_segment(&mut shapes, cursor, a, radius);
            if r > 0.01 {
                let center = Point::new(a.x + dout.x * r, a.y + dout.y * r);
                shapes.push(arc(center, a, b, r, radius));
            }
            cursor = b;
        }
        push_segment(&mut shapes, cursor, pts[pts.len() - 1], radius);
        shapes
    }
}

fn direction(a: Point, b: Point) -> Point {
    let len = a.distance(b).max(f32::EPSILON);
    Point::new((b.x - a.x) / len, (b.y - a.y) / len)
}

fn push_segment(shapes: &mut Vec<GooShape>, a: Point, b: Point, radius: f32) {
    if a.distance(b) > 0.01 {
        shapes.push(GooShape::Capsule { a, b, radius });
    }
}

fn arc(center: Point, a: Point, b: Point, r: f32, thickness: f32) -> GooShape {
    let start = (a.y - center.y).atan2(a.x - center.x);
    let end = (b.y - center.y).atan2(b.x - center.x);
    let mut sweep = end - start;
    while sweep > PI {
        sweep -= 2.0 * PI;
    }
    while sweep < -PI {
        sweep += 2.0 * PI;
    }
    let (start, sweep) = if sweep < 0.0 { (start + sweep, -sweep) } else { (start, sweep) };
    GooShape::Arc { center, radius: r, start, sweep, thickness }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_path_is_one_capsule() {
        let path = StepPath::horizontal(Point::new(0.0, 5.0), Point::new(40.0, 5.2), 20.0);
        assert_eq!(path.points().len(), 2);
        assert!(matches!(path.shapes(2.0, 6.0).as_slice(), [GooShape::Capsule { .. }]));
    }

    #[test]
    fn step_path_has_three_segments_and_two_arcs() {
        let path = StepPath::horizontal(Point::new(0.0, 0.0), Point::new(40.0, 30.0), 20.0);
        let shapes = path.shapes(2.0, 6.0);
        let arcs = shapes.iter().filter(|s| matches!(s, GooShape::Arc { .. })).count();
        assert_eq!((shapes.len(), arcs), (5, 2));
    }

    #[test]
    fn arcs_are_quarter_turns_with_positive_sweep() {
        let path = StepPath::horizontal(Point::new(0.0, 30.0), Point::new(40.0, 0.0), 20.0);
        for shape in path.shapes(2.0, 6.0) {
            if let GooShape::Arc { sweep, radius, .. } = shape {
                assert!((sweep - PI / 2.0).abs() < 1e-4);
                assert_eq!(radius, 6.0);
            }
        }
    }

    #[test]
    fn corner_shrinks_on_short_segments() {
        let path = StepPath::vertical(Point::new(0.0, 0.0), Point::new(4.0, 40.0), 20.0);
        let radii: Vec<f32> = path
            .shapes(2.0, 10.0)
            .into_iter()
            .filter_map(|s| if let GooShape::Arc { radius, .. } = s { Some(radius) } else { None })
            .collect();
        assert_eq!(radii, vec![2.0, 2.0]);
    }
}
