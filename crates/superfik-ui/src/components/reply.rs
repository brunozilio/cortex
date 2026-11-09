//! The assistant's reply: paragraphs, headings, bullets, numbered items and
//! code blocks, revealed word by word. Bold, `code` and links inside a line
//! come from `markdown`.

use std::sync::Arc;

use superfik_render::{Flow, Point, Quad, Rect, TextLayout, TextStyle};

use super::reply_table::Table;
use super::visual::Visual;
use crate::markdown::{self, Clean, Kind};
use crate::painter::Painter;
use crate::theme::{metrics, motion, typography};

pub type Reveal<'a> = &'a dyn Fn(usize) -> f32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReplyBlock<'a> {
    Paragraph(&'a str),
    Heading(&'a str),
    Code(&'a str),
    Bullet(&'a str),
    Numbered(&'a str, &'a str),
    Table(&'a [String], &'a [Vec<String>]),
    Visual { svg: &'a str, ratio: Option<f32>, complete: bool },
}

pub struct Reply<'a> {
    pub blocks: &'a [ReplyBlock<'a>],
    pub width: f32,
    pub reveal: Reveal<'a>,
}

impl std::fmt::Debug for Reply<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reply").field("blocks", &self.blocks).finish()
    }
}

const BLOCK_GAP: f32 = 9.5;
const BULLET_INDENT: f32 = 20.0;
const NUMBER_INDENT: f32 = 26.0;
const MARKER_GAP: f32 = 6.0;

struct Laid {
    layout: Arc<TextLayout>,
    clean: Clean,
}

impl Reply<'_> {
    fn block_width(&self, block: &ReplyBlock) -> f32 {
        match block {
            ReplyBlock::Paragraph(_) | ReplyBlock::Heading(_) | ReplyBlock::Table(..) | ReplyBlock::Visual { .. } => {
                self.width
            }
            ReplyBlock::Bullet(_) => self.width - BULLET_INDENT,
            ReplyBlock::Numbered(..) => self.width - NUMBER_INDENT,
            ReplyBlock::Code(_) => self.width - metrics::CODE_BLOCK_PAD * 2.0,
        }
    }

    fn style(block: &ReplyBlock) -> TextStyle {
        match block {
            ReplyBlock::Heading(_) => typography::HEADING,
            ReplyBlock::Code(_) => typography::CODE,
            _ => typography::BODY,
        }
    }

    fn lay(&self, p: &mut Painter, block: &ReplyBlock) -> Option<Laid> {
        let width = self.block_width(block);
        let clean = match block {
            ReplyBlock::Paragraph(text)
            | ReplyBlock::Heading(text)
            | ReplyBlock::Bullet(text)
            | ReplyBlock::Numbered(_, text) => markdown::clean(text),
            ReplyBlock::Code(text) => Clean { text: (*text).to_owned(), spans: Vec::new() },
            ReplyBlock::Table(..) | ReplyBlock::Visual { .. } => return None,
        };
        let layout = p.layout_styled(&clean.text, Self::style(block), Flow::plain(width), &[], &clean.runs());
        Some(Laid { layout, clean })
    }

    fn visual<'b>(&self, block: &ReplyBlock<'b>) -> Option<Visual<'b>> {
        match *block {
            ReplyBlock::Visual { svg, ratio, complete } => Some(Visual { svg, ratio, complete, width: self.width }),
            _ => None,
        }
    }

    fn table<'b>(&self, block: &ReplyBlock<'b>) -> Option<Table<'b>> {
        match block {
            ReplyBlock::Table(header, rows) => Some(Table { header, rows, width: self.width }),
            _ => None,
        }
    }

    pub fn height(&self, p: &mut Painter) -> f32 {
        let mut h = 0.0;
        for (i, block) in self.blocks.iter().enumerate() {
            let height = match (self.table(block), self.lay(p, block)) {
                _ if matches!(block, ReplyBlock::Visual { .. }) => self.visual(block).map_or(0.0, |v| v.height()),
                (Some(table), _) => table.height(p),
                (None, Some(laid)) if matches!(block, ReplyBlock::Code(_)) => {
                    laid.layout.height + metrics::CODE_BLOCK_PAD * 2.0
                }
                (None, Some(laid)) => laid.layout.height,
                (None, None) => 0.0,
            };
            h += height + if i > 0 { BLOCK_GAP } else { 0.0 };
        }
        h
    }

    pub fn paint(&self, p: &mut Painter, origin: Point, visuals: &mut Vec<Rect>) -> Rect {
        let mut y = origin.y;
        let mut words_before = 0;
        for (i, block) in self.blocks.iter().enumerate() {
            if i > 0 {
                y += BLOCK_GAP;
            }
            if let Some(visual) = self.visual(block) {
                let rect = visual.paint(p, Point::new(origin.x, y));
                visuals.push(rect);
                y += rect.h;
                continue;
            }
            if let Some(table) = self.table(block) {
                let (height, words) = table.paint(p, Point::new(origin.x, y), self.reveal, words_before);
                words_before += words;
                y += height;
                continue;
            }
            let Some(Laid { layout, clean }) = self.lay(p, block) else { continue };
            let pad = metrics::CODE_BLOCK_PAD;
            let x = match block {
                ReplyBlock::Paragraph(_) | ReplyBlock::Heading(_) => origin.x,
                ReplyBlock::Code(_) => {
                    let panel = Rect::new(origin.x, y, self.width, layout.height + pad * 2.0);
                    let fill = p.palette().text_body.faded(metrics::CODE_BLOCK_FILL * (self.reveal)(words_before));
                    p.quad(Quad::new(panel, metrics::ROW_RADIUS, fill));
                    y += pad;
                    origin.x + pad
                }
                ReplyBlock::Bullet(_) => {
                    let first = (self.reveal)(words_before);
                    let dot = Rect::new(origin.x + 4.0, y + 10.0, 4.0, 4.0);
                    p.quad(Quad::new(dot, 2.0, p.palette().text_tertiary.faded(first)));
                    origin.x + BULLET_INDENT
                }
                ReplyBlock::Numbered(marker, _) => {
                    let first = (self.reveal)(words_before);
                    let marker = p.layout(marker, typography::BODY, None);
                    let x = origin.x + NUMBER_INDENT - MARKER_GAP - marker.width;
                    p.text(&marker, Point::new(x, y), p.palette().text_tertiary.faded(first));
                    origin.x + NUMBER_INDENT
                }
                ReplyBlock::Table(..) | ReplyBlock::Visual { .. } => origin.x,
            };
            let base = words_before;
            let c = p.palette();
            let code = matches!(block, ReplyBlock::Code(_));
            p.text_with(&layout, Point::new(x, y), |offset| {
                let t = (self.reveal)(base + layout.word_index(offset));
                let color = match clean.kind_at(offset) {
                    _ if code => c.text_body,
                    Some(Kind::Code) => c.text_code,
                    Some(Kind::Link) => c.form_accent,
                    _ => c.text_body,
                };
                (color.faded(t), (1.0 - t) * motion::BLUR)
            });
            words_before += clean.text.split_whitespace().count();
            y += layout.height + if code { pad } else { 0.0 };
        }
        Rect::new(origin.x, origin.y, self.width, y - origin.y)
    }
}
