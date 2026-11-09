//! Visuals: SVG drawings an agent writes in its reply, inside a
//! ` ```visual ` fence, drawn inline in the conversation. This module reads
//! what the app needs from one without drawing it: its frame (the
//! `viewBox`) and the places a click asks something
//! (`onclick="sendPrompt('…')"`).

pub const FENCE: &str = "visual";

pub const WIDTH: f32 = 440.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Frame {
    pub fn ratio(&self) -> f32 {
        self.height / self.width
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    fn union(self, other: Area) -> Area {
        let (x, y) = (self.x.min(other.x), self.y.min(other.y));
        let right = (self.x + self.w).max(other.x + other.w);
        let bottom = (self.y + self.h).max(other.y + other.h);
        Area { x, y, w: right - x, h: bottom - y }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    pub area: Area,
    pub text: String,
}

pub fn frame(svg: &str) -> Option<Frame> {
    let start = svg.find("<svg")?;
    let tag = &svg[start..start + svg[start..].find('>')?];
    let numbers = |value: &str| -> Vec<f32> {
        value.split(|c: char| c.is_whitespace() || c == ',').filter_map(|n| n.parse().ok()).collect()
    };
    let frame = match attribute(tag, "viewBox").map(numbers).as_deref() {
        Some(&[x, y, width, height]) => Frame { x, y, width, height },
        _ => {
            let size = |name| attribute(tag, name).and_then(|v| v.trim_end_matches("px").parse::<f32>().ok());
            Frame { x: 0.0, y: 0.0, width: size("width")?, height: size("height")? }
        }
    };
    (frame.width > 0.0 && frame.height > 0.0).then_some(frame)
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let before = rest[..at].chars().next_back();
        let after = rest[at + name.len()..].trim_start();
        rest = &rest[at + name.len()..];
        if !before.is_some_and(char::is_whitespace) {
            continue;
        }
        let Some(value) = after.strip_prefix('=') else { continue };
        let value = value.trim_start();
        let quote = value.chars().next().filter(|q| *q == '"' || *q == '\'')?;
        let value = &value[1..];
        return value.find(quote).map(|end| &value[..end]);
    }
    None
}

pub fn sent_prompt(script: &str) -> Option<String> {
    let call = script.find("sendPrompt")?;
    let args = script[call + "sendPrompt".len()..].trim_start().strip_prefix('(')?.trim_start();
    let mut chars = args.chars();
    let quote = chars.next().filter(|q| matches!(q, '\'' | '"' | '`'))?;
    let mut text = String::new();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                'n' => text.push('\n'),
                't' => text.push('\t'),
                other => text.push(other),
            },
            c if c == quote => return Some(text).filter(|t| !t.trim().is_empty()),
            c => text.push(c),
        }
    }
    None
}

pub fn prompts(svg: &str) -> Vec<Prompt> {
    let Some(frame) = frame(svg) else { return Vec::new() };
    let Ok(document) = roxmltree::Document::parse(svg) else { return Vec::new() };
    document
        .descendants()
        .filter(|node| node.is_element())
        .filter_map(|node| {
            let text = sent_prompt(node.attribute("onclick")?)?;
            let (dx, dy) = node
                .ancestors()
                .skip(1)
                .try_fold((0.0, 0.0), |(x, y), a| translation(a).map(|(tx, ty)| (x + tx, y + ty)))?;
            let area = extent(node, dx, dy)?;
            Some(Prompt { area: Area { x: area.x - frame.x, y: area.y - frame.y, ..area }, text })
        })
        .collect()
}

fn translation(node: roxmltree::Node) -> Option<(f32, f32)> {
    let Some(transform) = node.attribute("transform") else { return Some((0.0, 0.0)) };
    let inner = transform.trim().strip_prefix("translate")?.trim_start().strip_prefix('(')?;
    let inner = inner.strip_suffix(')')?;
    let numbers: Vec<f32> =
        inner.split(|c: char| c.is_whitespace() || c == ',').filter_map(|n| n.parse().ok()).collect();
    match numbers.as_slice() {
        [x] => Some((*x, 0.0)),
        [x, y] => Some((*x, *y)),
        _ => None,
    }
}

fn extent(node: roxmltree::Node, dx: f32, dy: f32) -> Option<Area> {
    let (tx, ty) = translation(node)?;
    let (dx, dy) = (dx + tx, dy + ty);
    let number = |name| node.attribute(name).and_then(|v: &str| v.trim_end_matches("px").parse::<f32>().ok());
    let own = match node.tag_name().name() {
        "rect" => Some(Area {
            x: number("x").unwrap_or(0.0),
            y: number("y").unwrap_or(0.0),
            w: number("width")?,
            h: number("height")?,
        }),
        "circle" => {
            let r = number("r")?;
            Some(Area {
                x: number("cx").unwrap_or(0.0) - r,
                y: number("cy").unwrap_or(0.0) - r,
                w: r * 2.0,
                h: r * 2.0,
            })
        }
        "ellipse" => {
            let (rx, ry) = (number("rx")?, number("ry")?);
            Some(Area {
                x: number("cx").unwrap_or(0.0) - rx,
                y: number("cy").unwrap_or(0.0) - ry,
                w: rx * 2.0,
                h: ry * 2.0,
            })
        }
        "text" => text_extent(node, number("x").unwrap_or(0.0), number("y").unwrap_or(0.0)),
        _ => None,
    }
    .map(|a| Area { x: a.x + dx, y: a.y + dy, ..a });
    let leaf = node.tag_name().name() == "text";
    node.children()
        .filter(|c| c.is_element() && !leaf)
        .filter_map(|child| extent(child, dx, dy))
        .chain(own)
        .reduce(Area::union)
}

fn text_extent(node: roxmltree::Node, x: f32, y: f32) -> Option<Area> {
    let content: String = node.descendants().filter_map(|n| n.text()).collect();
    let chars = content.trim().chars().count();
    if chars == 0 {
        return None;
    }
    let classes = node.attribute("class").unwrap_or_default();
    let size = if classes.split_whitespace().any(|c| c == "ts") {
        12.0
    } else {
        node.attribute("font-size").and_then(|v| v.trim_end_matches("px").parse().ok()).unwrap_or(14.0)
    };
    let width = chars as f32 * size * 0.55;
    let left = match node.attribute("text-anchor") {
        Some("middle") => x - width / 2.0,
        Some("end") => x - width,
        _ => x,
    };
    let top = match node.attribute("dominant-baseline") {
        Some("central" | "middle") => y - size / 2.0,
        _ => y - size * 0.8,
    };
    Some(Area { x: left, y: top, w: width, h: size })
}

mod stream;
pub use stream::drawn_so_far;

#[cfg(test)]
mod tests;
