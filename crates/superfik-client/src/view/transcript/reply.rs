//! A turn's reply as the blocks the `Reply` component lays out, with
//! visuals still streaming in shown as far as they have arrived.

use superfik_core::{Block, Turn, visual};
use superfik_ui::components::ReplyBlock;

pub(super) fn blocks(turn: &Turn) -> Vec<Block> {
    turn.response.blocks()
}

pub(super) fn drawn(blocks: &[Block]) -> Vec<String> {
    let streaming = blocks.iter().filter_map(|b| match b {
        Block::Visual { svg, complete: false } => Some(svg),
        _ => None,
    });
    streaming.map(|svg| visual::drawn_so_far(svg).unwrap_or_default()).collect()
}

pub(super) fn as_reply<'a>(blocks: &'a [Block], drawn: &'a [String]) -> Vec<ReplyBlock<'a>> {
    let mut streaming = drawn.iter();
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(t) => ReplyBlock::Paragraph(t),
            Block::Table(header, rows) => ReplyBlock::Table(header, rows),
            Block::Heading(t) => ReplyBlock::Heading(t),
            Block::Code(t) => ReplyBlock::Code(t),
            Block::Bullet(t) => ReplyBlock::Bullet(t),
            Block::Numbered(marker, t) => ReplyBlock::Numbered(marker, t),
            Block::Visual { svg, complete } => {
                let ratio = visual::frame(svg).map(|f| f.ratio());
                let shown = if *complete { svg } else { streaming.next().map_or("", String::as_str) };
                ReplyBlock::Visual { svg: shown, ratio, complete: *complete }
            }
        })
        .collect()
}
