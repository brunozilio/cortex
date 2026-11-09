//! The user's message: optional thumbnails, a bubble and an optional
//! branch caption.

use superfik_render::{ImageId, Point, Quad, Rect};

use crate::icons;
use crate::painter::Painter;
use crate::theme::typography;

#[derive(Debug, Clone, Copy)]
pub struct UserBubble<'a> {
    pub text: &'a str,
    pub images: &'a [ImageId],
    pub caption: &'a str,
    pub max_width: f32,
}

const PAD_X: f32 = 16.0;
const PAD_Y: f32 = 9.5;
const THUMB: f32 = 42.0;
const THUMB_GAP: f32 = 6.5;
const THUMB_RADIUS: f32 = 6.0;
const CAPTION_GAP: f32 = 10.0;

fn bubble_width(text_width: f32, lines: usize, max_width: f32) -> f32 {
    if lines > 1 { max_width } else { (text_width + PAD_X * 2.0).min(max_width) }
}

impl UserBubble<'_> {
    fn text_width(&self) -> f32 {
        self.max_width - PAD_X * 2.0
    }

    pub fn height(&self, p: &mut Painter) -> f32 {
        let layout = p.layout(self.text, typography::BUBBLE, Some(self.text_width()));
        let thumbs = if self.images.is_empty() { 0.0 } else { THUMB + THUMB_GAP + 2.0 };
        let caption = if self.caption.is_empty() { 0.0 } else { CAPTION_GAP + 14.0 };
        thumbs + layout.height + PAD_Y * 2.0 + caption
    }

    pub fn width(&self, p: &mut Painter) -> f32 {
        p.layout(self.text, typography::BUBBLE, Some(self.text_width())).width + PAD_X * 2.0
    }

    pub fn thumbs_height(&self) -> f32 {
        if self.images.is_empty() { 0.0 } else { THUMB + 8.0 }
    }

    pub fn paint(&self, p: &mut Painter, right: f32, top: f32) {
        let mut y = top;
        if !self.images.is_empty() {
            for (i, image) in self.images.iter().rev().enumerate() {
                let rect = Rect::new(right - THUMB - i as f32 * (THUMB + THUMB_GAP), y, THUMB, THUMB);
                p.quad(Quad::new(rect, THUMB_RADIUS, p.palette().well));
                p.image(*image, rect, THUMB_RADIUS, 0.0);
            }
            y += THUMB + THUMB_GAP + 2.0;
        }
        let layout = p.layout(self.text, typography::BUBBLE, Some(self.text_width()));
        let w = bubble_width(layout.width, layout.lines.len(), self.max_width);
        let bubble = Rect::new(right - w, y, w, layout.height + PAD_Y * 2.0);
        p.quad(Quad::new(bubble, 10.0, p.palette().bubble));
        p.text(&layout, Point::new(bubble.x + PAD_X, bubble.y + PAD_Y), p.palette().text_body);
        if self.caption.is_empty() {
            return;
        }
        let cy = bubble.bottom() + CAPTION_GAP + 7.0;
        let caption = p.layout(self.caption, typography::BRANCH_CAPTION, None);
        let x = right - caption.width;
        p.text(&caption, Point::new(x, (cy - caption.height * 0.5).round()), p.palette().text_tertiary);
        p.icon_at(icons::BRANCH, Point::new(x - 8.0, cy), 10.0, Some(p.palette().text_tertiary));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_messages_fill_the_bubble() {
        assert_eq!(bubble_width(300.0, 2, 372.0), 372.0);
        assert_eq!(bubble_width(30.0, 1, 372.0), 30.0 + PAD_X * 2.0);
    }
}
