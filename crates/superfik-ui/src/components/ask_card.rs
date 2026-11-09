//! The ask card: what a paused agent wants from the user, painted where the
//! composer sits. A header like the composer's (emote, title, counter), a
//! well with the subject or question and its options, and a row of reply
//! buttons. It grows upward from the composer's bottom edge.

use superfik_render::{GooPrim, GooShape, Point, Quad, Rect};

use super::emote::Emote;
use super::sheet::Button;
use crate::emote::EmoteState;
use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, motion, typography};

#[derive(Debug, Clone, Copy)]
pub struct AskOption<'a> {
    pub label: &'a str,
    pub description: &'a str,
    pub picked: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct AskButton<'a> {
    pub label: &'a str,
    pub key: &'a str,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskPart {
    Option(usize),
    Button(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct AskCardProps<'a> {
    pub x: f32,
    pub bottom: f32,
    pub emote: EmoteState,
    pub emote_from: Option<(EmoteState, f32)>,
    pub title: &'a str,
    pub counter: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub question: Option<&'a str>,
    pub options: &'a [AskOption<'a>],
    pub cursor: Option<usize>,
    pub hint: Option<&'a str>,
    pub buttons: &'a [AskButton<'a>],
    pub hovered: Option<AskPart>,
    pub progress: f32,
    pub time: f32,
}

#[derive(Debug, Clone, Default)]
pub struct AskCardHits {
    pub body: Rect,
    pub options: Vec<Rect>,
    pub buttons: Vec<Rect>,
}

#[derive(Debug, Clone, Copy)]
pub struct AskCard<'a> {
    pub props: AskCardProps<'a>,
}

impl AskCard<'_> {
    const WIDTH: f32 = metrics::COMPOSER_WIDTH;

    pub fn height(&self, p: &mut Painter) -> f32 {
        (metrics::ASK_HEADER + self.well_height(p) + Self::footer_height()).max(metrics::COMPOSER_HEIGHT)
    }

    fn footer_height() -> f32 {
        metrics::ASK_FOOTER_GAP + metrics::CHIP_HEIGHT + metrics::ASK_INSET
    }

    pub fn paint(&self, p: &mut Painter) -> AskCardHits {
        let props = &self.props;
        let h = self.height(p);
        let k = motion::ENTER.apply(props.progress.clamp(0.0, 1.0));
        let top = props.bottom - h + metrics::ASK_RISE * (1.0 - k);
        let body = Rect::new(props.x, top, Self::WIDTH, h);
        p.faded(k, |p| {
            let shape = GooShape::RoundRect { rect: body, radius: metrics::NODE_RADIUS };
            p.surface(
                GooPrim::new(shape, p.palette().surface_top, metrics::GOO)
                    .gradient(p.palette().surface_top, p.palette().surface_bottom),
            );
            self.paint_header(p, body);
            let well_h = h - metrics::ASK_HEADER - Self::footer_height();
            let well = Rect::new(
                body.x + metrics::ASK_INSET,
                body.y + metrics::ASK_HEADER,
                body.w - metrics::ASK_INSET * 2.0,
                well_h,
            );
            let options = self.paint_well(p, well);
            let buttons = self.paint_footer(p, body, well.bottom() + metrics::ASK_FOOTER_GAP);
            AskCardHits { body, options, buttons }
        })
    }

    fn paint_header(&self, p: &mut Painter, body: Rect) {
        let props = &self.props;
        let (tile_w, tile_h) = metrics::ORB_TILE;
        let tile = Rect::new(body.x + 15.0, body.y + 10.0, tile_w, tile_h);
        p.quad(Quad::new(tile, 6.0, p.palette().control));
        p.clipped(tile, |p| {
            Emote { state: props.emote, from: props.emote_from, time: props.time, width: metrics::COMPOSER_EMOTE }
                .paint(p, tile.center())
        });
        let cy = body.y + metrics::ASK_HEADER * 0.5;
        let right = body.right() - 13.0;
        let counter = match props.counter {
            Some(text) => super::Badge { text }.paint(p, right, cy).w + 8.0,
            None => 0.0,
        };
        let x = tile.right() + 12.0;
        p.label_fit(props.title, typography::MODEL, x, cy, right - counter - x, p.palette().text);
    }

    fn well_height(&self, p: &mut Painter) -> f32 {
        let props = &self.props;
        let width = Self::text_width();
        let mut h = metrics::ASK_WELL_PADDING * 2.0;
        if let Some(subject) = props.subject {
            let max = metrics::ASK_SUBJECT_LINES * typography::CODE.line_height;
            h += p.layout(subject, typography::CODE, Some(width)).height.min(max);
        }
        if let Some(question) = props.question {
            h += p.layout(question, typography::INPUT, Some(width)).height;
        }
        if !props.options.is_empty() {
            let n = props.options.len() as f32;
            h += metrics::ASK_WELL_PADDING * 0.5 + n * metrics::ASK_OPTION + (n - 1.0) * metrics::ASK_OPTION_GAP;
        }
        h
    }

    fn text_width() -> f32 {
        Self::WIDTH - metrics::ASK_INSET * 2.0 - metrics::ASK_WELL_PADDING * 2.0
    }

    fn paint_well(&self, p: &mut Painter, well: Rect) -> Vec<Rect> {
        let props = &self.props;
        p.well(GooShape::RoundRect { rect: well, radius: metrics::WELL_RADIUS }, p.palette().well, 6.0);
        let pad = metrics::ASK_WELL_PADDING;
        let width = Self::text_width();
        let mut y = well.y + pad;
        if let Some(subject) = props.subject {
            let layout = p.layout(subject, typography::CODE, Some(width));
            let shown = layout.height.min(metrics::ASK_SUBJECT_LINES * typography::CODE.line_height);
            let area = Rect::new(well.x + pad, y, width, shown);
            p.clipped(area, |p| p.text(&layout, Point::new(area.x, area.y), p.palette().text));
            y += shown;
        }
        if let Some(question) = props.question {
            let layout = p.layout(question, typography::INPUT, Some(width));
            p.text(&layout, Point::new(well.x + pad, y), p.palette().text);
            y += layout.height + pad * 0.5;
        }
        let row_x = well.x + pad * 0.5;
        let row_w = well.w - pad;
        props
            .options
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let row = Rect::new(row_x, y, row_w, metrics::ASK_OPTION);
                y += metrics::ASK_OPTION + metrics::ASK_OPTION_GAP;
                self.paint_option(p, row, i, option)
            })
            .collect()
    }

    fn paint_option(&self, p: &mut Painter, row: Rect, index: usize, option: &AskOption) -> Rect {
        let c = p.palette();
        let lit = self.props.hovered == Some(AskPart::Option(index)) || self.props.cursor == Some(index);
        if lit || option.picked {
            p.quad(Quad::new(row, metrics::ROW_RADIUS, if lit { c.hover } else { c.control }));
        }
        let cy = row.y + row.h * 0.5;
        let key = Rect::new(row.x + 8.0, cy - metrics::ASK_KEY * 0.5, metrics::ASK_KEY, metrics::ASK_KEY);
        let digit = (index + 1).to_string();
        p.quad(Quad::new(key, metrics::BADGE_RADIUS, if option.picked { c.send } else { c.badge }));
        let digit_color = if option.picked { c.send_glyph } else { c.text_secondary };
        let layout = p.layout(&digit, typography::CAPTION, None);
        p.label(&digit, typography::CAPTION, key.x + (key.w - layout.width) * 0.5, cy, digit_color);
        let x = key.right() + 10.0;
        let trailing = if option.picked {
            p.icon_at(icons::CHECK_CIRCLE, Point::new(row.right() - 14.0, cy), 13.0, Some(c.icon));
            28.0
        } else {
            10.0
        };
        let max = row.right() - trailing - x;
        if option.description.is_empty() {
            p.label_fit(option.label, typography::MENU, x, cy, max, c.text);
        } else {
            p.label_fit(option.label, typography::MENU, x, cy - 8.0, max, c.text);
            p.label_fit(option.description, typography::CAPTION, x, cy + 9.0, max, c.text_secondary);
        }
        row
    }

    fn paint_footer(&self, p: &mut Painter, body: Rect, y: f32) -> Vec<Rect> {
        let props = &self.props;
        let labels: Vec<String> = props
            .buttons
            .iter()
            .map(|b| if b.key.is_empty() { b.label.to_owned() } else { format!("{}  {}", b.label, b.key) })
            .collect();
        let mut right = body.right() - metrics::ASK_INSET;
        let mut rects = vec![Rect::default(); props.buttons.len()];
        for (i, (button, label)) in props.buttons.iter().zip(&labels).enumerate().rev() {
            let b = Button {
                label,
                primary: button.primary,
                hovered: props.hovered == Some(AskPart::Button(i)),
                enabled: true,
            };
            let w = b.width(p);
            rects[i] = b.paint(p, Rect::new(right - w, y, w, metrics::CHIP_HEIGHT));
            right -= w + metrics::ASK_BUTTON_GAP;
        }
        if let Some(hint) = props.hint {
            let x = body.x + metrics::ASK_INSET + 4.0;
            let cy = y + metrics::CHIP_HEIGHT * 0.5;
            p.label_fit(hint, typography::CAPTION, x, cy, right - x, p.palette().text_tertiary);
        }
        rects
    }
}
