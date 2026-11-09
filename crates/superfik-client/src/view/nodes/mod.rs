//! One module per canvas node. Each turns plain props into components and
//! registers its click targets; none of them reads the session directly.

pub mod agent;
pub mod browser;
pub mod commands;
pub mod connections;
pub mod context;
pub mod memory;
pub mod notes;
pub mod paged;
pub mod pull_request;
pub mod references;
pub mod repository;
pub mod research;
pub mod skills;

use superfik_i18n::Strings;
use superfik_render::Rect;
use superfik_ui::Painter;
use superfik_ui::theme::metrics;

use super::target::{HitMap, Target};

#[derive(Debug)]
pub struct NodeCtx<'a> {
    pub rect: Rect,
    pub content: f32,
    pub busy: bool,
    pub time: f32,
    pub hovered: Option<Target>,
    pub page_slide: f32,
    pub strings: Strings,
    pub hits: &'a mut HitMap,
}

impl NodeCtx<'_> {
    pub fn is_hovered(&self, target: Target) -> bool {
        self.hovered == Some(target)
    }

    pub fn body(&self) -> Rect {
        let p = metrics::NODE_PADDING;
        Rect::new(
            self.rect.x + p,
            self.rect.y + metrics::NODE_HEADER,
            self.rect.w - p * 2.0,
            self.rect.h - metrics::NODE_HEADER - p,
        )
    }
}

pub fn reveal<R>(p: &mut Painter, content: f32, f: impl FnOnce(&mut Painter) -> R) -> R {
    let blur = (1.0 - content) * superfik_ui::theme::motion::BLUR;
    p.faded(content, |p| p.blurred(blur, f))
}

pub fn rows_height(count: usize, row: f32, gap: f32) -> f32 {
    if count == 0 { 0.0 } else { count as f32 * row + (count - 1) as f32 * gap }
}

pub fn node_height(body: f32) -> f32 {
    metrics::NODE_HEADER + body + metrics::NODE_PADDING
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_match_reference_nodes() {
        assert_eq!(rows_height(2, 30.0, 7.0), 67.0);
        assert_eq!(rows_height(0, 30.0, 7.0), 0.0);
        assert_eq!(node_height(66.0), 116.0);
    }
}
