//! Rows of a long node a page at a time (Skills, Plugins): the page slides
//! in from the side it was flipped towards, and a pager sits underneath.

use superfik_render::Rect;
use superfik_ui::Painter;
use superfik_ui::components::Pager;
use superfik_ui::theme::metrics;

use super::{NodeCtx, rows_height};
use crate::state::{Carousel, page_count};
use crate::view::target::Target;

pub fn body_height(rows: usize, row: f32) -> f32 {
    let pager = if rows > metrics::PAGE_ROWS { metrics::ROW_GAP + metrics::PAGER_HEIGHT } else { 0.0 };
    rows_height(rows.min(metrics::PAGE_ROWS), row, metrics::ROW_GAP) + pager
}

pub fn row_top(top: f32, index: usize, row: f32) -> f32 {
    top + (index % metrics::PAGE_ROWS) as f32 * (row + metrics::ROW_GAP)
}

pub fn paint(
    p: &mut Painter,
    ctx: &mut NodeCtx,
    carousel: Carousel,
    (rows, row, page): (usize, f32, usize),
    mut paint_row: impl FnMut(&mut Painter, &mut NodeCtx, usize, Rect),
) {
    ctx.hits.carousels.push((ctx.rect, carousel));
    let body = ctx.body();
    let shown = Rect::new(body.x, body.y, body.w, rows_height(rows.min(metrics::PAGE_ROWS), row, metrics::ROW_GAP));
    let (dx, settle) = (ctx.page_slide * body.w, 1.0 - ctx.page_slide.abs());
    let first = page * metrics::PAGE_ROWS;
    p.clipped(shown, |p| {
        p.faded(settle, |p| {
            for index in (first..rows).take(metrics::PAGE_ROWS) {
                let rect = Rect::new(body.x + dx, row_top(body.y, index, row), body.w, row);
                paint_row(p, ctx, index, rect);
            }
        });
    });
    let pages = page_count(rows);
    if pages < 2 {
        return;
    }
    let bar = Rect::new(body.x, shown.bottom() + metrics::ROW_GAP, body.w, metrics::PAGER_HEIGHT);
    let pager = Pager {
        page,
        pages,
        hovered_prev: ctx.is_hovered(Target::Page(carousel, -1)),
        hovered_next: ctx.is_hovered(Target::Page(carousel, 1)),
    }
    .paint(p, bar);
    for (rect, by) in [(pager.prev, -1), (pager.next, 1)] {
        if let Some(rect) = rect {
            ctx.hits.add(rect, Target::Page(carousel, by));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_list_is_a_page_tall_with_a_pager() {
        assert_eq!(body_height(2, 30.0), rows_height(2, 30.0, metrics::ROW_GAP));
        let page = rows_height(metrics::PAGE_ROWS, 30.0, metrics::ROW_GAP);
        assert_eq!(body_height(metrics::PAGE_ROWS, 30.0), page);
        assert_eq!(body_height(40, 30.0), page + metrics::ROW_GAP + metrics::PAGER_HEIGHT);
    }

    #[test]
    fn rows_sit_on_their_page() {
        assert_eq!(row_top(100.0, 0, 30.0), 100.0);
        assert_eq!(row_top(100.0, metrics::PAGE_ROWS + 1, 30.0), row_top(100.0, 1, 30.0));
    }
}
