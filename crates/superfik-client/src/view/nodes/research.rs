//! Research node: sources found on the web, with skeletons while searching.

use superfik_core::{ResearchItem, ResearchKind};
use superfik_i18n::Text;
use superfik_render::{Icon, Quad, Rect};
use superfik_ui::components::{NodeFrame, SkeletonRow};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, reveal, rows_height};

const ROWS: usize = 4;
const GAP: f32 = 5.0;
const SKELETON_WIDTHS: [f32; ROWS] = [0.5, 0.92, 0.88, 0.8];

pub fn height() -> f32 {
    node_height(rows_height(ROWS, metrics::ROW_HEIGHT, GAP))
}

fn icon(kind: ResearchKind) -> Icon {
    match kind {
        ResearchKind::Article => icons::BOOKMARK,
        ResearchKind::Document => icons::DOCUMENT,
        ResearchKind::Discussion => icons::CHAT,
        ResearchKind::Video => icons::PLAY,
    }
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, items: &[ResearchItem]) {
    let count = items.len().to_string();
    let title = ctx.strings.t(Text::Research);
    let mut frame = NodeFrame::new(ctx.rect, icons::GLOBE_FILLED, Some(p.palette().research), &title);
    frame.count = Some(&count);
    frame.busy = ctx.busy;
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.paint(p);
    let body = ctx.body();
    reveal(p, ctx.content, |p| {
        for (i, width) in SKELETON_WIDTHS.into_iter().enumerate() {
            let row = Rect::new(body.x, body.y + i as f32 * (metrics::ROW_HEIGHT + GAP), body.w, metrics::ROW_HEIGHT);
            match items.get(i) {
                Some(item) => paint_item(p, row, item),
                None => SkeletonRow { width }.paint(p, row),
            }
        }
    });
}

fn paint_item(p: &mut Painter, row: Rect, item: &ResearchItem) {
    p.quad(Quad::new(row, metrics::ROW_RADIUS, p.palette().well));
    let cy = row.center().y;
    let size = metrics::RESEARCH_TILE;
    let tile = Rect::new(row.x + 5.0, cy - size * 0.5, size, size);
    p.quad(Quad::new(tile, metrics::RESEARCH_TILE_RADIUS, p.palette().research_tile));
    p.icon_at(icon(item.kind), tile.center(), metrics::RESEARCH_GLYPH, Some(p.palette().research));
    let x = tile.right() + 7.5;
    p.label_fit(&item.title, typography::LABEL, x, cy, row.right() - 8.0 - x, p.palette().text_label);
}
