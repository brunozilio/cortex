//! References node: images and files that guide the work. Clicking a tile
//! opens it in the full-window viewer; each tile carries a remove knob.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use superfik_i18n::Text;
use superfik_ui::components::{EarSide, ImageGrid, NodeFrame, Tile};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, reveal};
use crate::view::LoadedImage;
use crate::view::layout::NodeKey;
use crate::view::target::Target;

pub fn height(count: usize) -> f32 {
    node_height(ImageGrid::height(metrics::NODE_WIDTH - metrics::NODE_PADDING * 2.0, count))
}

pub fn tile<'a>(path: &'a Path, images: &HashMap<PathBuf, LoadedImage>) -> Tile<'a> {
    match images.get(path) {
        Some(image) => Tile::Image { id: image.id, aspect: image.aspect },
        None => Tile::File { name: path.file_name().and_then(|n| n.to_str()).unwrap_or("file") },
    }
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, tiles: &[Tile], viewing: Option<usize>, used: bool) {
    let count = tiles.len().to_string();
    let title = ctx.strings.t(Text::References);
    let mut frame = NodeFrame::new(ctx.rect, icons::IMAGE, Some(p.palette().reference), &title);
    frame.count = (!used).then_some(count.as_str());
    frame.ear = Some(EarSide::Left);
    frame.ear_hovered = ctx.is_hovered(Target::Close(NodeKey::References));
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.busy = ctx.busy;
    let hits = frame.paint(p);
    if let Some(knob) = hits.knob {
        ctx.hits.add(knob, Target::Close(NodeKey::References));
    }
    let body = ctx.body();
    let grid = ImageGrid {
        tiles,
        selected: viewing.filter(|i| *i < tiles.len()),
        hovered: match ctx.hovered {
            Some(Target::Reference(i)) => Some(i),
            _ => None,
        },
        hovered_remove: match ctx.hovered {
            Some(Target::RemoveReference(i)) => Some(i),
            _ => None,
        },
        hovered_add: ctx.is_hovered(Target::AddReference),
    };
    let hits = reveal(p, ctx.content, |p| grid.paint(p, body));
    ctx.hits.viewed_tile = grid.selected.and_then(|i| hits.tiles.get(i).copied());
    for (index, rect) in hits.tiles.iter().enumerate() {
        ctx.hits.add(*rect, Target::Reference(index));
    }
    for (index, knob) in hits.removes.iter().enumerate() {
        if let Some(knob) = knob {
            ctx.hits.add(*knob, Target::RemoveReference(index));
        }
    }
    ctx.hits.add(hits.add, Target::AddReference);
}
