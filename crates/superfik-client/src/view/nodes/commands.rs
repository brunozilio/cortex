//! Commands node: the `/commands` the composer understands, a page at a
//! time. A row runs its command.

use superfik_core::Command;
use superfik_i18n::Text;
use superfik_render::Rect;
use superfik_ui::components::{NodeFrame, Row};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, paged, reveal};
use crate::state::Carousel;
use crate::view::target::Target;

const ROW: f32 = 34.0;

pub fn height() -> f32 {
    node_height(paged::body_height(Command::ALL.len(), ROW))
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, page: usize) {
    let count = Command::ALL.len().to_string();
    let title = ctx.strings.t(Text::Commands);
    let mut frame = NodeFrame::new(ctx.rect, icons::BOLT, Some(p.palette().skill_bolt), &title);
    frame.count = Some(&count);
    frame.busy = ctx.busy;
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.paint(p);
    reveal(p, ctx.content, |p| {
        paged::paint(p, ctx, Carousel::Commands, (Command::ALL.len(), ROW, page), |p, ctx, i, row| {
            paint_row(p, ctx, Command::ALL[i], i, row);
        });
    });
}

fn paint_row(p: &mut Painter, ctx: &mut NodeCtx, command: Command, index: usize, row: Rect) {
    let label = format!("/{}  {}", command.name(), ctx.strings.command(command));
    Row::new(&label)
        .icon(icons::BOLT, Some(p.palette().skill_bolt))
        .icon_size(metrics::SKILL_BOLT, metrics::SKILL_ICON + metrics::SKILL_ICON_GAP - metrics::SKILL_BOLT)
        .style(typography::SKILL)
        .color(p.palette().text)
        .hovered(ctx.is_hovered(Target::Command(index)))
        .paint(p, row);
    ctx.hits.add(row, Target::Command(index));
}
