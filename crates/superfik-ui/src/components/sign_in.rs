//! A screen over the whole window, laid out like Get Started's page (the
//! lock screen): the emote, the title, a line under it, why the last try
//! failed, the main button (Unlock with Touch ID) and, under it, an
//! optional quieter one.
//! Stateless: the caller says what the buttons read and whether they work.

use superfik_render::{Point, Quad, Rect};

use super::{Button, Emote};
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy)]
pub struct SignIn<'a> {
    pub emote: Emote,
    pub title: &'a str,
    pub caption: &'a str,
    pub error: Option<&'a str>,
    pub primary: Button<'a>,
    pub secondary: Option<Button<'a>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SignInHits {
    pub primary: Rect,
    pub secondary: Option<Rect>,
}

impl SignIn<'_> {
    pub fn paint(&self, p: &mut Painter, area: Rect) -> SignInHits {
        let c = p.palette();
        p.quad(Quad::new(area, 0.0, c.canvas));
        let width = metrics::SIGN_IN_WIDTH.min(area.w - metrics::SCREEN_PADDING * 2.0).max(0.0);
        let caption = p.layout(self.caption, typography::STORE_BODY, Some(width));
        let error = self.error.map(|error| p.layout(error, typography::STORE_BODY, Some(width)));
        let (button_w, button_h) = metrics::GET_STARTED_BUTTON;
        let secondary_h = self.secondary.map_or(0.0, |_| metrics::SIGN_IN_GAP + metrics::CHIP_HEIGHT);
        let height = metrics::GET_STARTED_EMOTE
            + metrics::GET_STARTED_EMOTE_GAP
            + typography::GET_STARTED_TITLE.line_height
            + metrics::SIGN_IN_GAP
            + caption.height
            + error.as_ref().map_or(0.0, |error| metrics::SIGN_IN_GAP + error.height)
            + metrics::GET_STARTED_ACTION_GAP
            + button_h
            + secondary_h;
        let cx = area.center().x;
        let mut y = area.y + ((area.h - height) * 0.5).max(metrics::SCREEN_PADDING);
        let emote = Emote { width: metrics::GET_STARTED_EMOTE, ..self.emote };
        emote.paint(p, Point::new(cx, y + metrics::GET_STARTED_EMOTE * 0.5));
        y += metrics::GET_STARTED_EMOTE + metrics::GET_STARTED_EMOTE_GAP;
        let title = p.layout(self.title, typography::GET_STARTED_TITLE, None);
        p.label(
            self.title,
            typography::GET_STARTED_TITLE,
            cx - title.width * 0.5,
            y + typography::GET_STARTED_TITLE.line_height * 0.5,
            c.text,
        );
        y += typography::GET_STARTED_TITLE.line_height + metrics::SIGN_IN_GAP;
        p.text_centered(&caption, cx, y, c.text_secondary);
        y += caption.height;
        if let Some(error) = &error {
            y += metrics::SIGN_IN_GAP;
            p.text_centered(error, cx, y, c.danger);
            y += error.height;
        }
        y += metrics::GET_STARTED_ACTION_GAP;
        let primary = self.primary.paint(p, Rect::new(cx - button_w * 0.5, y, button_w, button_h));
        y += button_h + metrics::SIGN_IN_GAP;
        let secondary = self.secondary.map(|button| {
            let w = button.width(p);
            button.paint(p, Rect::new(cx - w * 0.5, y, w, metrics::CHIP_HEIGHT))
        });
        SignInHits { primary, secondary }
    }
}
