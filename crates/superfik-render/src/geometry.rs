//! Logical-pixel geometry.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    pub fn distance(self, other: Point) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const EVERYTHING: Rect = Rect { x: -1.0e6, y: -1.0e6, w: 2.0e6, h: 2.0e6 };

    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_points(a: Point, b: Point) -> Self {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    pub fn inset(&self, by: f32) -> Self {
        Self::new(self.x + by, self.y + by, (self.w - 2.0 * by).max(0.0), (self.h - 2.0 * by).max(0.0))
    }

    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    pub fn intersect(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        Rect::new(x, y, (r - x).max(0.0), (b - y).max(0.0))
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(x, y, self.right().max(other.right()) - x, self.bottom().max(other.bottom()) - y)
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x <= other.right() && other.x <= self.right() && self.y <= other.bottom() && other.y <= self.bottom()
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.w, self.h]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_math() {
        let r = Rect::new(10.0, 10.0, 20.0, 10.0);
        assert_eq!(r.center(), Point::new(20.0, 15.0));
        assert!(r.contains(Point::new(30.0, 20.0)));
        assert!(!r.contains(Point::new(31.0, 20.0)));
        assert_eq!(r.inset(2.0), Rect::new(12.0, 12.0, 16.0, 6.0));
        assert_eq!(r.inset(50.0).w, 0.0);
    }

    #[test]
    fn intersect_union_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersect(&b), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&Rect::new(20.0, 0.0, 1.0, 1.0)));
        assert_eq!(Rect::from_points(Point::new(4.0, 1.0), Point::new(1.0, 3.0)), Rect::new(1.0, 1.0, 3.0, 2.0));
    }
}
