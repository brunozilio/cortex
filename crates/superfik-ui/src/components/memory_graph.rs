//! The memory graph: one dot per memory scattered inside a brain-shaped
//! outline, joined by dotted links. A search lights the dots it reaches
//! and leaves the rest unlit.
//!
//! Positions are stable: a memory's place comes from its id (the id-th
//! point of a Halton sequence that falls inside the outline), so adding
//! or forgetting memories never moves the others.

use superfik_render::{Color, Point, Quad, Rect};

use crate::painter::Painter;
use crate::theme::{colors::MemoryTones, metrics};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphDot {
    pub seed: u32,
    pub tone: Color,
    pub radius: f32,
    pub light: f32,
    pub grow: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryGraph<'a> {
    pub dots: &'a [GraphDot],
    pub links: &'a [(usize, usize)],
    pub searching: bool,
    pub tones: MemoryTones,
    pub view: GraphView,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphView {
    pub pan: Point,
    pub zoom: f32,
}

impl Default for GraphView {
    fn default() -> Self {
        Self { pan: Point::new(0.0, 0.0), zoom: 1.0 }
    }
}

impl GraphView {
    pub fn apply(&self, point: Point, rect: Rect) -> Point {
        let c = rect.center();
        Point::new(c.x + (point.x - c.x) * self.zoom + self.pan.x, c.y + (point.y - c.y) * self.zoom + self.pan.y)
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32, rect: Rect) {
        self.pan = Point::new(self.pan.x + dx, self.pan.y + dy);
        self.clamp(rect);
    }

    pub fn zoom_at(&mut self, factor: f32, at: Point, rect: Rect) {
        let zoom = (self.zoom * factor).clamp(metrics::MEMORY_ZOOM.0, metrics::MEMORY_ZOOM.1);
        let c = rect.center();
        let ratio = zoom / self.zoom;
        let (ax, ay) = (at.x - c.x, at.y - c.y);
        self.pan = Point::new(ax - (ax - self.pan.x) * ratio, ay - (ay - self.pan.y) * ratio);
        self.zoom = zoom;
        self.clamp(rect);
    }

    fn clamp(&mut self, rect: Rect) {
        let (x, y) = (rect.w * 0.5 * self.zoom, rect.h * 0.5 * self.zoom);
        self.pan = Point::new(self.pan.x.clamp(-x, x), self.pan.y.clamp(-y, y));
    }
}

fn inside(p: Point) -> bool {
    let ellipse = |cx: f32, cy: f32, rx: f32, ry: f32| ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2) <= 1.0;
    ellipse(0.48, 0.44, 0.47, 0.38) || ellipse(0.7, 0.76, 0.2, 0.15)
}

fn halton(mut index: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while index > 0 {
        f /= base as f32;
        r += f * (index % base) as f32;
        index /= base;
    }
    r
}

pub fn places(seeds: &[u32]) -> Vec<Point> {
    let highest = seeds.iter().copied().max().unwrap_or(0) as usize;
    let mut accepted = Vec::with_capacity(highest + 1);
    let mut index = 1;
    while accepted.len() <= highest {
        let p = Point::new(halton(index, 2), halton(index, 3));
        if inside(p) {
            accepted.push(p);
        }
        index += 1;
    }
    seeds.iter().map(|&seed| accepted[seed as usize]).collect()
}

pub fn mesh(points: &[Point], extra: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut pairs: Vec<(usize, usize)> = extra.iter().map(|&(a, b)| (a.min(b), a.max(b))).collect();
    for (i, p) in points.iter().enumerate() {
        let mut nearest: Vec<(f32, usize)> =
            points.iter().enumerate().filter(|(j, _)| *j != i).map(|(j, q)| (p.distance(*q), j)).collect();
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        pairs.extend(nearest.iter().take(2).map(|&(_, j)| (i.min(j), i.max(j))));
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

impl MemoryGraph<'_> {
    pub fn paint(&self, p: &mut Painter, rect: Rect) -> Vec<Rect> {
        let seeds: Vec<u32> = self.dots.iter().map(|d| d.seed).collect();
        let area = fit(rect.inset(metrics::MEMORY_GRAPH_PADDING), metrics::MEMORY_GRAPH_ASPECT);
        let points: Vec<Point> = places(&seeds)
            .into_iter()
            .map(|u| self.view.apply(Point::new(area.x + u.x * area.w, area.y + u.y * area.h), rect))
            .collect();
        let room = (area.w * area.h / self.dots.len().max(1) as f32).sqrt();
        let scale = (room / 26.0).clamp(0.55, metrics::MEMORY_DOT_MAX_SCALE) * self.view.zoom.sqrt();
        p.clipped(rect, |p| self.paint_points(p, rect, &points, scale))
    }

    fn paint_points(&self, p: &mut Painter, rect: Rect, points: &[Point], scale: f32) -> Vec<Rect> {
        let lit = |i: usize| !self.searching || self.dots[i].light > 0.0;
        for (a, b) in mesh(points, self.links) {
            let color = match (self.searching, lit(a) && lit(b)) {
                (true, true) => self.tones.line_lit,
                (true, false) => self.tones.unlit,
                (false, _) => self.tones.line,
            };
            dotted(p, points[a], points[b], color);
        }
        let mut hits = Vec::with_capacity(self.dots.len());
        for (dot, center) in self.dots.iter().zip(points) {
            let color =
                if self.searching { self.tones.unlit.mix(dot.tone, dot.light.clamp(0.0, 1.0)) } else { dot.tone };
            let light = if self.searching { dot.light } else { 0.0 };
            let r = (dot.radius + metrics::MEMORY_DOT_GROW * dot.grow.max(light * 0.6)) * scale;
            p.quad(Quad::new(Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0), r, color));
            let reach = r.max(6.0);
            hits.push(Rect::new(center.x - reach, center.y - reach, reach * 2.0, reach * 2.0).intersect(&rect));
        }
        hits
    }
}

fn fit(rect: Rect, aspect: f32) -> Rect {
    let (w, h) = if rect.w / rect.h > aspect { (rect.h * aspect, rect.h) } else { (rect.w, rect.w / aspect) };
    Rect::new(rect.x + (rect.w - w) * 0.5, rect.y + (rect.h - h) * 0.5, w, h)
}

fn dotted(p: &mut Painter, a: Point, b: Point, color: Color) {
    let length = a.distance(b);
    let step = metrics::MEMORY_LINE * 4.0;
    let size = metrics::MEMORY_LINE * 1.6;
    let count = (length / step).floor() as usize;
    for k in 1..count {
        let t = k as f32 / count as f32;
        let at = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        p.quad(Quad::new(Rect::new(at.x - size * 0.5, at.y - size * 0.5, size, size), size * 0.5, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_stable_and_inside_the_outline() {
        let all = places(&[0, 1, 2, 3, 4, 5]);
        let some = places(&[2, 5]);
        assert_eq!(some, vec![all[2], all[5]], "a memory keeps its place whatever else exists");
        assert!(all.iter().all(|p| inside(*p)));
        let mut unique = all.clone();
        unique.dedup();
        assert_eq!(unique.len(), all.len());
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let rect = Rect::new(0.0, 0.0, 200.0, 100.0);
        let mut view = GraphView::default();
        let at = Point::new(150.0, 30.0);
        view.zoom_at(2.0, at, rect);
        assert_eq!(view.zoom, 2.0);
        let other = view.apply(Point::new(20.0, 80.0), rect);
        assert!((other.x - 20.0).abs() > 1.0, "other points move");
        let under = Point::new(100.0 + (150.0 - 100.0 - view.pan.x) / 2.0, 50.0 + (30.0 - 50.0 - view.pan.y) / 2.0);
        let back = view.apply(under, rect);
        assert!((back.x - at.x).abs() < 0.01 && (back.y - at.y).abs() < 0.01);
        view.zoom_at(100.0, at, rect);
        assert_eq!(view.zoom, metrics::MEMORY_ZOOM.1);
        view.pan_by(10_000.0, 0.0, rect);
        assert_eq!(view.pan.x, rect.w * 0.5 * view.zoom, "the graph never leaves");
    }

    #[test]
    fn the_outline_keeps_its_aspect() {
        let wide = fit(Rect::new(0.0, 0.0, 400.0, 100.0), 1.5);
        assert_eq!((wide.w, wide.h, wide.x), (150.0, 100.0, 125.0));
        let tall = fit(Rect::new(0.0, 0.0, 150.0, 400.0), 1.5);
        assert_eq!((tall.w, tall.h, tall.y), (150.0, 100.0, 150.0));
    }

    #[test]
    fn mesh_joins_neighbours_and_keeps_meaningful_links() {
        let points = [Point::new(0.0, 0.0), Point::new(1.0, 0.0), Point::new(2.0, 0.0), Point::new(9.0, 9.0)];
        let pairs = mesh(&points, &[(3, 0)]);
        assert!(pairs.contains(&(0, 1)) && pairs.contains(&(1, 2)) && pairs.contains(&(0, 3)));
        assert!(pairs.windows(2).all(|w| w[0] < w[1]), "sorted and unique");
        assert!(mesh(&[], &[]).is_empty());
    }
}
