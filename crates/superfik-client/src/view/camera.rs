//! The canvas camera: two-finger panning and pinch zoom anchored at the
//! pointer. Canvas content is laid out in canvas coordinates; the screen
//! shows `(canvas + pan) * zoom`. Pure and tested.

use superfik_motion::{Spring, SpringConfig};
use superfik_render::{Point, Rect};

pub const MIN_ZOOM: f32 = 0.4;
pub const MAX_ZOOM: f32 = 2.5;
pub const KEY_STEP: f32 = 1.2;
const ZOOM_STEPS: f32 = 50.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    pub pan: Point,
    pub zoom: f32,
    raw: f32,
    glide: Option<(Spring, Spring)>,
    shift: Spring,
    room: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::at(Point::default(), 1.0)
    }
}

impl Camera {
    pub fn offset(&self) -> Point {
        Point::new(self.pan.x + self.shift.value, self.pan.y)
    }

    pub fn to_canvas(&self, screen: Point) -> Point {
        let pan = self.offset();
        Point::new(screen.x / self.zoom - pan.x, screen.y / self.zoom - pan.y)
    }

    pub fn to_screen(&self, rect: Rect) -> Rect {
        let pan = self.offset();
        Rect::new((rect.x + pan.x) * self.zoom, (rect.y + pan.y) * self.zoom, rect.w * self.zoom, rect.h * self.zoom)
    }

    pub fn rect_to_canvas(&self, screen: Rect) -> Rect {
        let a = self.to_canvas(Point::new(screen.x, screen.y));
        let b = self.to_canvas(Point::new(screen.right(), screen.bottom()));
        Rect::from_points(a, b)
    }

    pub fn make_room(&mut self, canvas_x: Option<f32>, screen_x: f32) {
        self.room = canvas_x.map_or(0.0, |x| (screen_x / self.zoom - x - self.pan.x).max(0.0));
    }

    pub fn fence(&mut self, canvas_x: f32, screen_x: f32) {
        let least = screen_x / self.zoom - canvas_x;
        if self.pan.x < least {
            self.pan.x = least;
            if let Some((x, _)) = &mut self.glide {
                *x = Spring::new(least, SpringConfig::LAYOUT);
                x.set_target(least);
            }
        }
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.glide = None;
        self.pan = Point::new(self.pan.x + dx / self.zoom, self.pan.y + dy / self.zoom);
    }

    pub fn zoom_at(&mut self, factor: f32, screen: Point) {
        self.glide = None;
        let anchor = self.to_canvas(screen);
        self.raw = (self.raw * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.zoom = (self.raw * ZOOM_STEPS).round() / ZOOM_STEPS;
        self.pan = Point::new(screen.x / self.zoom - anchor.x, screen.y / self.zoom - anchor.y);
    }

    pub fn at(pan: Point, zoom: f32) -> Self {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        Self { pan, zoom, raw: zoom, glide: None, shift: Spring::new(0.0, SpringConfig::LAYOUT), room: 0.0 }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn centre_on(&mut self, rect: Rect, area: Rect) {
        let target =
            Point::new(area.center().x / self.zoom - rect.center().x, area.center().y / self.zoom - rect.center().y);
        let spring = |value: f32, to: f32| {
            let mut spring = Spring::new(value, SpringConfig::LAYOUT);
            spring.set_target(to);
            spring
        };
        self.glide = Some((spring(self.pan.x, target.x), spring(self.pan.y, target.y)));
    }

    pub fn reveal(&mut self, rect: Rect, keep: Rect, area: Rect, margin: f32) {
        let shown = self.to_screen(rect).inset(-margin);
        let inside =
            shown.x >= area.x && shown.y >= area.y && shown.right() <= area.right() && shown.bottom() <= area.bottom();
        if inside {
            return;
        }
        let x = rect.x.min(keep.x);
        let both = Rect::new(x, rect.y, rect.right().max(keep.right()) - x, rect.h);
        let fits = ((area.w - margin * 2.0) / both.w).min((area.h - margin * 2.0) / both.h);
        if fits < self.zoom {
            let zoom = ((fits * ZOOM_STEPS).floor() / ZOOM_STEPS).clamp(MIN_ZOOM, MAX_ZOOM);
            let center = Point::new(area.x + area.w * 0.5, area.y + area.h * 0.5);
            let anchor = self.to_canvas(center);
            self.zoom = zoom;
            self.raw = zoom;
            self.pan = Point::new(center.x / zoom - anchor.x, center.y / zoom - anchor.y);
        }
        self.glide_into(rect, area, margin);
    }

    fn glide_into(&mut self, rect: Rect, area: Rect, margin: f32) {
        let shown = self.to_screen(rect);
        let shift = |start: f32, length: f32, from: f32, size: f32| {
            let (low, high) = (from + margin, from + size - margin);
            if length > high - low || start < low {
                low - start
            } else if start + length > high {
                high - (start + length)
            } else {
                0.0
            }
        };
        let dx = shift(shown.x, shown.w, area.x, area.w) / self.zoom;
        let dy = shift(shown.y, shown.h, area.y, area.h) / self.zoom;
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let spring = |value: f32, by: f32| {
            let mut spring = Spring::new(value, SpringConfig::LAYOUT);
            spring.set_target(value + by);
            spring
        };
        self.glide = Some((spring(self.pan.x, dx), spring(self.pan.y, dy)));
    }

    pub fn step(&mut self, dt: f32) {
        self.shift.set_target(self.room);
        self.shift.step(dt);
        if let Some((x, y)) = &mut self.glide {
            x.step(dt);
            y.step(dt);
            self.pan = Point::new(x.value, y.value);
            if x.is_settled() && y.is_settled() {
                self.pan = Point::new(x.target, y.target);
                self.glide = None;
            }
        }
    }

    #[cfg(test)]
    pub fn settle(&mut self) {
        if let Some((x, y)) = self.glide.take() {
            self.pan = Point::new(x.target, y.target);
        }
    }

    pub fn is_animating(&self) -> bool {
        self.glide.is_some() || !self.shift.is_settled()
    }
}

#[cfg(test)]
mod tests;
