//! A thin drawing context over a [`Scene`] used by every component.

use std::sync::Arc;

use superfik_render::{
    Color, Flow, GlyphSprite, GooPrim, GooShape, GooStyle, Icon, IconSprite, ImageId, ImageSprite, Picture,
    PictureSprite, Point, Quad, Rect, Scene, StyleRun, TextLayout, TextStyle, TextSystem,
};

use crate::theme::colors::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    Backdrop,
    Canvas,
    Overlay,
}

impl Plane {
    const ALL: [Self; 3] = [Self::Backdrop, Self::Canvas, Self::Overlay];

    const fn base(self) -> usize {
        match self {
            Self::Backdrop => 0,
            Self::Canvas => 2,
            Self::Overlay => 4,
        }
    }
}

pub const LAYERS: usize = 6;

pub struct Painter<'a> {
    scene: &'a mut Scene,
    text: &'a mut TextSystem,
    scale: f32,
    plane: Plane,
    clip: Rect,
    opacity: f32,
    blur: f32,
    offset: Point,
    palette: &'static Palette,
    elided: Vec<Elided>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Elided {
    pub rect: Rect,
    pub text: String,
}

impl std::fmt::Debug for Painter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Painter").field("plane", &self.plane).field("opacity", &self.opacity).finish()
    }
}

impl<'a> Painter<'a> {
    pub fn new(scene: &'a mut Scene, text: &'a mut TextSystem, scale: f32, palette: &'static Palette) -> Self {
        scene.pattern_tones = palette.patterns;
        for plane in Plane::ALL {
            let base = plane.base();
            scene.layer(base).goo_style = GooStyle {
                highlight: palette.surface_highlight,
                shadow: palette.surface_shadow,
                shadow_offset: 1.5,
                shadow_blur: 3.5,
            };
            scene.layer(base + 1).goo_style = GooStyle {
                highlight: palette.well_rim,
                shadow: Color::TRANSPARENT,
                shadow_offset: 0.0,
                shadow_blur: 0.0,
            };
        }
        Self {
            scene,
            text,
            scale,
            plane: Plane::Canvas,
            clip: Rect::EVERYTHING,
            opacity: 1.0,
            blur: 0.0,
            offset: Point::default(),
            palette,
            elided: Vec::new(),
        }
    }

    pub fn palette(&self) -> &'static Palette {
        self.palette
    }

    pub fn set_offset(&mut self, offset: Point) {
        self.offset = offset;
    }

    fn moved(&self, rect: Rect) -> Rect {
        rect.translate(self.offset.x, self.offset.y)
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    pub fn with_palette<R>(&mut self, palette: &'static Palette, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = std::mem::replace(&mut self.palette, palette);
        let result = f(self);
        self.palette = previous;
        result
    }

    pub fn on_plane<R>(&mut self, plane: Plane, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = std::mem::replace(&mut self.plane, plane);
        let result = f(self);
        self.plane = previous;
        result
    }

    pub fn faded<R>(&mut self, opacity: f32, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.opacity;
        self.opacity *= opacity.clamp(0.0, 1.0);
        let result = f(self);
        self.opacity = previous;
        result
    }

    pub fn blurred<R>(&mut self, blur: f32, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.blur;
        self.blur += blur.max(0.0);
        let result = f(self);
        self.blur = previous;
        result
    }

    pub fn clipped<R>(&mut self, clip: Rect, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.clip;
        self.clip = previous.intersect(&self.moved(clip));
        let result = f(self);
        self.clip = previous;
        result
    }

    pub fn surface(&mut self, prim: GooPrim) {
        let mut prim = prim.opacity(prim.opacity * self.opacity);
        prim.shape = prim.shape.translate(self.offset.x, self.offset.y);
        self.scene.layer(self.plane.base()).goo.push(prim);
    }

    pub fn surface_rect(&mut self, rect: Rect, radius: f32, smooth: f32) {
        let shape = GooShape::RoundRect { rect, radius };
        let c = self.palette;
        self.surface(GooPrim::new(shape, c.surface_top, smooth).gradient(c.surface_top, c.surface_bottom));
    }

    pub fn well(&mut self, shape: GooShape, color: Color, smooth: f32) {
        let shape = shape.translate(self.offset.x, self.offset.y);
        let prim = GooPrim::new(shape, color, smooth).opacity(self.opacity);
        self.scene.layer(self.plane.base() + 1).goo.push(prim);
    }

    pub fn quad(&mut self, quad: Quad) {
        let mut quad = quad.opacity(quad.opacity * self.opacity);
        quad.rect = self.moved(quad.rect);
        let quad = quad.clip(self.moved(quad.clip).intersect(&self.clip));
        self.scene.layer(self.plane.base() + 1).quads.push(quad);
    }

    pub fn icon(&mut self, icon: Icon, rect: Rect, tint: Option<Color>) {
        if self.opacity <= 0.0 {
            return;
        }
        let sprite = IconSprite { icon, rect: self.moved(rect), tint, opacity: self.opacity, clip: self.clip };
        self.scene.layer(self.plane.base() + 1).icons.push(sprite);
    }

    pub fn icon_at(&mut self, icon: Icon, center: Point, size: f32, tint: Option<Color>) {
        let rect = Rect::new(center.x - size * 0.5, center.y - size * 0.5, size, size);
        self.icon(icon, rect, tint);
    }

    pub fn image(&mut self, image: ImageId, rect: Rect, radius: f32, blur: f32) {
        let sprite = ImageSprite {
            image,
            rect: self.moved(rect),
            radius,
            opacity: self.opacity,
            blur: blur + self.blur,
            clip: self.clip,
        };
        self.scene.layer(self.plane.base() + 1).images.push(sprite);
    }

    pub fn picture(&mut self, picture: Picture, rect: Rect, radius: f32) {
        let sprite = PictureSprite { picture, rect: self.moved(rect), radius, opacity: self.opacity, clip: self.clip };
        self.scene.layer(self.plane.base() + 1).pictures.push(sprite);
    }

    pub fn layout(&mut self, text: &str, style: TextStyle, max_width: Option<f32>) -> Arc<TextLayout> {
        self.text.layout(text, style, max_width)
    }

    pub fn layout_flow(&mut self, text: &str, style: TextStyle, flow: Flow) -> Arc<TextLayout> {
        self.text.layout_flow(text, style, flow)
    }

    pub fn layout_spaced(
        &mut self,
        text: &str,
        style: TextStyle,
        flow: Flow,
        spaced: &[(usize, f32)],
    ) -> Arc<TextLayout> {
        self.text.layout_spaced(text, style, flow, spaced)
    }

    pub fn layout_styled(
        &mut self,
        text: &str,
        style: TextStyle,
        flow: Flow,
        spaced: &[(usize, f32)],
        runs: &[StyleRun],
    ) -> Arc<TextLayout> {
        self.text.layout_styled(text, style, flow, spaced, runs)
    }

    pub fn text(&mut self, layout: &TextLayout, origin: Point, color: Color) {
        self.text_with(layout, origin, |_| (color, 0.0));
    }

    pub fn text_with(&mut self, layout: &TextLayout, origin: Point, style: impl Fn(usize) -> (Color, f32)) {
        self.lines(layout, origin, |_| 0.0, style);
    }

    pub fn text_centered(&mut self, layout: &TextLayout, center_x: f32, top: f32, color: Color) {
        let left = center_x - layout.width * 0.5;
        let width = layout.width;
        self.lines(layout, Point::new(left, top), |line_width| ((width - line_width) * 0.5).round(), |_| (color, 0.0));
    }

    fn lines(
        &mut self,
        layout: &TextLayout,
        origin: Point,
        indent: impl Fn(f32) -> f32,
        style: impl Fn(usize) -> (Color, f32),
    ) {
        let layer = self.plane.base() + 1;
        let origin = Point::new(origin.x + self.offset.x, origin.y + self.offset.y);
        for line in &layout.lines {
            let origin = Point::new(origin.x + indent(line.width), origin.y);
            for glyph in &line.glyphs {
                let (color, blur) = style(glyph.start);
                let color = color.faded(self.opacity);
                if color.a <= 0.003 {
                    continue;
                }
                let offset = (origin.x * self.scale, ((origin.y + line.baseline) * self.scale).round());
                let physical = glyph.physical(offset, self.scale);
                self.scene.layer(layer).glyphs.push(GlyphSprite {
                    key: physical.cache_key,
                    x: physical.x,
                    y: physical.y,
                    color,
                    blur: blur + self.blur,
                    clip: self.clip,
                });
            }
        }
    }

    pub fn fit(&mut self, text: &str, style: TextStyle, max_width: f32) -> String {
        if self.layout(text, style, None).width <= max_width {
            return text.to_owned();
        }
        let chars: Vec<char> = text.chars().collect();
        let (mut lo, mut hi) = (0, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate: String = chars[..mid].iter().collect::<String>().trim_end().to_owned() + "…";
            if self.layout(&candidate, style, None).width <= max_width { lo = mid } else { hi = mid - 1 }
        }
        chars[..lo].iter().collect::<String>().trim_end().to_owned() + "…"
    }

    pub fn label_fit(&mut self, text: &str, style: TextStyle, x: f32, center_y: f32, max_width: f32, color: Color) {
        self.label_fit_for(text, text, style, (x, center_y), max_width, color);
    }

    pub fn label_fit_for(
        &mut self,
        text: &str,
        full: &str,
        style: TextStyle,
        (x, center_y): (f32, f32),
        max_width: f32,
        color: Color,
    ) {
        let fitted = self.fit(text, style, max_width);
        let layout = self.label(&fitted, style, x, center_y, color);
        if fitted != full {
            let rect = Rect::new(x, (center_y - layout.height * 0.5).round(), layout.width, layout.height);
            self.note_elided(rect, full);
        }
    }

    pub fn note_elided(&mut self, rect: Rect, full: &str) {
        let rect = self.moved(rect).intersect(&self.clip);
        if self.opacity <= 0.0 || rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        self.elided.push(Elided { rect, text: full.to_owned() });
    }

    pub fn take_elided(&mut self) -> Vec<Elided> {
        std::mem::take(&mut self.elided)
    }

    pub fn label(&mut self, text: &str, style: TextStyle, x: f32, center_y: f32, color: Color) -> Arc<TextLayout> {
        let layout = self.layout(text, style, None);
        self.text(&layout, Point::new(x, (center_y - layout.height * 0.5).round()), color);
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{colors, typography};

    fn elided(paint: impl FnOnce(&mut Painter)) -> Vec<Elided> {
        let mut scene = Scene::new(Color::TRANSPARENT, LAYERS);
        let mut text = TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &colors::DARK);
        paint(&mut p);
        p.take_elided()
    }

    const LONG: &str = "a label far too long for the room it is given";

    #[test]
    fn a_shortened_label_is_noted_with_its_whole_text() {
        let notes = elided(|p| p.label_fit(LONG, typography::LABEL, 10.0, 20.0, 60.0, Color::WHITE));
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, LONG);
        let rect = notes[0].rect;
        assert!(rect.x == 10.0 && rect.w <= 60.0 && rect.contains(Point::new(15.0, 20.0)), "{rect:?}");
    }

    #[test]
    fn a_label_that_fits_is_not_noted() {
        assert!(elided(|p| p.label_fit("short", typography::LABEL, 0.0, 10.0, 400.0, Color::WHITE)).is_empty());
    }

    #[test]
    fn a_pre_shortened_label_is_noted_with_the_original() {
        let notes =
            elided(|p| p.label_fit_for("…/b/c.rs", "a/b/c.rs", typography::LABEL, (0.0, 10.0), 400.0, Color::WHITE));
        assert_eq!(notes.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["a/b/c.rs"]);
    }

    #[test]
    fn notes_follow_the_offset_and_skip_hidden_labels() {
        let notes = elided(|p| {
            p.set_offset(Point::new(100.0, 0.0));
            p.label_fit(LONG, typography::LABEL, 0.0, 10.0, 60.0, Color::WHITE);
            p.faded(0.0, |p| p.label_fit(LONG, typography::LABEL, 0.0, 40.0, 60.0, Color::WHITE));
            p.clipped(Rect::new(0.0, 100.0, 10.0, 10.0), |p| {
                p.label_fit(LONG, typography::LABEL, 0.0, 10.0, 60.0, Color::WHITE);
            });
        });
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].rect.x, 100.0);
    }
}
