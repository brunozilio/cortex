//! An agent's visual in the conversation: an SVG drawing set in the
//! theme's colours (`themed`) and drawn one SVG unit to one point. While it
//! streams in it shows what has arrived, or a shimmering place for it
//! until the first shape does.

use std::hash::{Hash, Hasher};

use superfik_render::{Color, Fill, Pattern, Picture, Point, Quad, Rect};

use crate::painter::Painter;
use crate::theme::visual::{self, RAMPS, Ramp, VisualTones, stop};

#[derive(Debug, Clone, Copy)]
pub struct Visual<'a> {
    pub svg: &'a str,
    pub ratio: Option<f32>,
    pub complete: bool,
    pub width: f32,
}

impl Visual<'_> {
    pub fn height(&self) -> f32 {
        self.ratio.map_or(visual::PENDING_HEIGHT, |ratio| self.width * ratio)
    }

    pub fn paint(&self, p: &mut Painter, origin: Point) -> Rect {
        let rect = Rect::new(origin.x, origin.y, self.width, self.height());
        if self.complete || !self.svg.is_empty() {
            let svg = themed(self.svg, &p.palette().visual);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            svg.hash(&mut hasher);
            p.picture(Picture { key: hasher.finish(), svg: svg.into() }, rect, visual::FRAME_RADIUS);
        } else {
            let well = p.palette().well;
            p.quad(Quad::new(rect, visual::FRAME_RADIUS, well).fill(Fill::solid(well)).pattern(Pattern::Shimmer));
        }
        rect
    }
}

pub fn themed(svg: &str, tones: &VisualTones) -> String {
    let svg = resolve_variables(svg, tones);
    let Some(start) = svg.find("<svg") else { return svg };
    let Some(end) = svg[start..].find('>').map(|at| start + at) else { return svg };
    let tag = &svg[start..end];
    let self_closing = tag.ends_with('/');
    let mut root = strip_attribute(&strip_attribute(tag.trim_end_matches('/'), "width"), "height");
    if !root.contains("xmlns=") {
        root.push_str(r#" xmlns="http://www.w3.org/2000/svg""#);
    }
    let close = if self_closing { "/>" } else { ">" };
    let style = if self_closing { String::new() } else { format!("<style>{}</style>", stylesheet(tones)) };
    format!("{}{root}{close}{style}{}", &svg[..start], &svg[end + 1..])
}

fn strip_attribute(tag: &str, name: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let before = rest[..at].chars().next_back();
        let after = rest[at + name.len()..].trim_start();
        let value = after.strip_prefix('=').map(str::trim_start);
        let quote = value.and_then(|v| v.chars().next()).filter(|q| *q == '"' || *q == '\'');
        match (before.filter(|c| c.is_whitespace()), value, quote) {
            (Some(_), Some(value), Some(q)) => match value[1..].find(q) {
                Some(close) => {
                    out.push_str(rest[..at].trim_end());
                    let consumed = rest.len() - value.len() + 1 + close + 1;
                    rest = &rest[consumed..];
                }
                None => break,
            },
            _ => {
                out.push_str(&rest[..at + name.len()]);
                rest = &rest[at + name.len()..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn css(color: Color) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if color.a >= 1.0 {
        format!("#{:02x}{:02x}{:02x}", byte(color.r), byte(color.g), byte(color.b))
    } else {
        format!("rgba({},{},{},{:.3})", byte(color.r), byte(color.g), byte(color.b), color.a)
    }
}

fn stops(ramp: &Ramp, dark: bool) -> [Color; 4] {
    let s = ramp.stops;
    if dark {
        [s[stop::S800], s[stop::S200], s[stop::S100], s[stop::S200]]
    } else {
        [s[stop::S50], s[stop::S600], s[stop::S800], s[stop::S600]]
    }
}

fn variable(name: &str, tones: &VisualTones) -> Option<String> {
    let semantic = |ramp: &Ramp, light: usize, dark: usize| css(ramp.stops[if tones.dark { dark } else { light }]);
    let semantic_ramp = |kind: &str| match kind {
        "info" => Some(&visual::BLUE),
        "danger" => Some(&visual::RED),
        "success" => Some(&visual::GREEN),
        "warning" => Some(&visual::AMBER),
        _ => None,
    };
    let value = match name {
        "p" | "color-text-primary" => css(tones.text),
        "s" | "color-text-secondary" => css(tones.text_secondary),
        "t" | "color-text-tertiary" => css(tones.text_tertiary),
        "bg2" | "color-background-secondary" => css(tones.surface),
        "color-background-primary" | "color-background-tertiary" => css(tones.page),
        "b" | "color-border-tertiary" => css(tones.border),
        "color-border-secondary" | "color-border-primary" => css(tones.border_strong),
        "font-sans" | "font-serif" => "Inter, sans-serif".into(),
        "font-mono" => "'Fira Mono', monospace".into(),
        "border-radius-md" => format!("{}px", visual::RADII[0]),
        "border-radius-lg" => format!("{}px", visual::RADII[1]),
        "border-radius-xl" => format!("{}px", visual::RADII[2]),
        _ => {
            let (group, kind) = name.strip_prefix("color-")?.split_once('-')?;
            let ramp = semantic_ramp(kind)?;
            match group {
                "text" => semantic(ramp, stop::S600, stop::S200),
                "background" => semantic(ramp, stop::S50, stop::S800),
                "border" => semantic(ramp, stop::S400, stop::S400),
                _ => return None,
            }
        }
    };
    Some(value)
}

fn resolve_variables(svg: &str, tones: &VisualTones) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(at) = rest.find("var(--") {
        let Some(close) = rest[at..].find(')') else { break };
        out.push_str(&rest[..at]);
        let inner = &rest[at + "var(--".len()..at + close];
        let (name, fallback) = match inner.split_once(',') {
            Some((name, fallback)) => (name.trim(), Some(fallback.trim())),
            None => (inner.trim(), None),
        };
        let value = variable(name, tones).or_else(|| fallback.map(str::to_owned)).unwrap_or_else(|| css(tones.text));
        out.push_str(&value);
        rest = &rest[at + close + 1..];
    }
    out.push_str(rest);
    out
}

fn stylesheet(tones: &VisualTones) -> String {
    let (text, secondary, tertiary) = (css(tones.text), css(tones.text_secondary), css(tones.text_tertiary));
    let (label, small) = (visual::LABEL, visual::LABEL_SMALL);
    let mut sheet = format!(
        "text{{font-family:Inter,sans-serif;fill:{text}}}\
         .t{{font-size:{label}px;fill:{text}}}\
         .th{{font-size:{label}px;font-weight:500;fill:{text}}}\
         .ts{{font-size:{small}px;fill:{secondary}}}\
         .box{{fill:{surface};stroke:{border}}}\
         .arr{{fill:none;stroke:{secondary};stroke-width:1.5px}}\
         .leader{{fill:none;stroke:{tertiary};stroke-width:0.5px;stroke-dasharray:3 3}}",
        surface = css(tones.surface),
        border = css(tones.border),
    );
    for ramp in &RAMPS {
        let [fill, stroke, title, subtitle] = stops(ramp, tones.dark).map(css);
        let c = ramp.name;
        sheet.push_str(&format!(
            ".c-{c}>rect,.c-{c}>circle,.c-{c}>ellipse,.c-{c}>polygon,rect.c-{c},circle.c-{c},ellipse.c-{c},polygon.c-{c}\
             {{fill:{fill};stroke:{stroke}}}\
             .c-{c}>.t,.c-{c}>.th{{fill:{title}}}.c-{c}>.ts{{fill:{subtitle}}}"
        ));
    }
    sheet
}

#[cfg(test)]
mod tests;
