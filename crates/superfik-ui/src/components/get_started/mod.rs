//! Get Started, a short film then a page. The window darkens into a night
//! stage where a greeting and the cast play (`intro`); the cast leaves, the
//! emote flies up to sit above the title and the stage lightens into the
//! canvas; then the title, the feature cards growing one after another
//! (`cards`) and one primary button. Timing lives in `choreography`.
//!
//! The backdrop is painted on the canvas plane and everything else on the
//! overlay plane, so the cards' surfaces sit above it.

mod cards;
mod choreography;
mod intro;

use superfik_render::{Color, Icon, Point, Quad, Rect};

use self::cards::Card;
pub use self::choreography::Beat;
use self::intro::Stage;
use super::emote::Emote;
use super::sheet::Button;
use crate::painter::{Painter, Plane};
use crate::theme::{colors, metrics, motion, typography};

#[derive(Debug, Clone, Copy)]
pub struct Feature<'a> {
    pub icon: Icon,
    pub tint: Color,
    pub title: &'a str,
    pub body: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct GetStarted<'a> {
    pub welcome: &'a str,
    pub emote: Emote,
    pub title: &'a str,
    pub features: &'a [Feature<'a>],
    pub action: &'a str,
    pub action_hovered: bool,
    pub elapsed: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GetStartedHits {
    pub action: Option<Rect>,
}

struct Page {
    emote: Point,
    title_y: f32,
    x: f32,
    width: f32,
    cards: Vec<(Card, Rect)>,
    action: Rect,
}

impl GetStarted<'_> {
    pub fn paint(&self, p: &mut Painter, area: Rect) -> GetStartedHits {
        let beat = Beat(self.elapsed);
        paint_backdrop(p, area, beat);
        let page = self.layout(p, area);
        let stage = Stage::new(area);
        p.on_plane(Plane::Overlay, |p| {
            intro::paint(p, &stage, self.welcome, beat);
            self.paint_emote(p, &stage, &page, beat);
            self.paint_page(p, &page, beat)
        })
    }

    fn layout(&self, p: &mut Painter, area: Rect) -> Page {
        let width = metrics::GET_STARTED_WIDTH.min(area.w - metrics::SCREEN_PADDING * 2.0).max(0.0);
        let measured: Vec<Card> = self.features.iter().map(|f| Card::measure(p, f, width)).collect();
        let gaps = measured.len().saturating_sub(1) as f32 * metrics::GET_STARTED_ROW_GAP;
        let (button_w, button_h) = metrics::GET_STARTED_BUTTON;
        let height = metrics::GET_STARTED_EMOTE
            + metrics::GET_STARTED_EMOTE_GAP
            + typography::GET_STARTED_TITLE.line_height
            + metrics::GET_STARTED_TITLE_GAP
            + measured.iter().map(|c| c.height).sum::<f32>()
            + gaps
            + metrics::GET_STARTED_ACTION_GAP
            + button_h;
        let cx = area.center().x;
        let x = cx - width * 0.5;
        let mut y = area.y + ((area.h - height) * 0.5).max(0.0);
        let emote = Point::new(cx, y + metrics::GET_STARTED_EMOTE * 0.5);
        y += metrics::GET_STARTED_EMOTE + metrics::GET_STARTED_EMOTE_GAP;
        let title_y = y;
        y += typography::GET_STARTED_TITLE.line_height + metrics::GET_STARTED_TITLE_GAP;
        let mut cards = Vec::with_capacity(measured.len());
        for card in measured {
            let rect = Rect::new(x, y, width, card.height);
            y += card.height + metrics::GET_STARTED_ROW_GAP;
            cards.push((card, rect));
        }
        y += metrics::GET_STARTED_ACTION_GAP - metrics::GET_STARTED_ROW_GAP;
        Page { emote, title_y, x, width, cards, action: Rect::new(cx - button_w * 0.5, y, button_w, button_h) }
    }

    fn paint_emote(&self, p: &mut Painter, stage: &Stage, page: &Page, beat: Beat) {
        let pop = beat.cast(intro::EMOTE_AT);
        if pop <= 0.0 {
            return;
        }
        let from = stage.member(intro::EMOTE_AT);
        let f = beat.flight();
        let lerp = |a: f32, b: f32| a + (b - a) * f;
        let start = metrics::GET_STARTED_CAST_EMOTE;
        let width = lerp(start, metrics::GET_STARTED_EMOTE)
            * (metrics::GET_STARTED_CAST_FROM + (1.0 - metrics::GET_STARTED_CAST_FROM) * pop);
        let center = Point::new(lerp(from.x, page.emote.x), lerp(from.y, page.emote.y));
        let emote = Emote { width, ..self.emote };
        let palette = if beat.lift() < 0.5 { &colors::DARK } else { p.palette() };
        p.with_palette(palette, |p| {
            p.faded(pop, |p| p.blurred((1.0 - pop) * motion::BLUR, |p| emote.paint(p, center)));
        });
    }

    fn paint_page(&self, p: &mut Painter, page: &Page, beat: Beat) -> GetStartedHits {
        reveal(p, beat.title(), |p, rise| {
            let title = p.layout(self.title, typography::GET_STARTED_TITLE, None);
            let x = (page.x + page.width * 0.5 - title.width * 0.5).round();
            p.text(&title, Point::new(x, page.title_y + rise), p.palette().text);
        });
        for (i, (feature, (card, rect))) in self.features.iter().zip(&page.cards).enumerate() {
            cards::paint(p, feature, card, *rect, beat.card(i), beat.content(i));
        }
        let shown = beat.action(self.features.len());
        let button = Button { label: self.action, primary: true, hovered: self.action_hovered, enabled: true };
        reveal(p, shown, |p, rise| {
            button.paint(p, page.action.translate(0.0, rise));
        });
        GetStartedHits { action: (shown > 0.0).then_some(page.action) }
    }
}

fn paint_backdrop(p: &mut Painter, area: Rect, beat: Beat) {
    let (dark, lift) = (beat.darken(), beat.lift());
    let c = p.palette();
    p.quad(Quad::new(area, 0.0, c.canvas.faded(dark)));
    p.quad(Quad::new(area, 0.0, c.stage.faded(dark * (1.0 - lift))));
}

fn reveal(p: &mut Painter, t: f32, f: impl FnOnce(&mut Painter, f32)) {
    if t <= 0.0 {
        return;
    }
    let left = 1.0 - t;
    p.faded(t, |p| p.blurred(left * motion::BLUR, |p| f(p, left * metrics::GET_STARTED_RISE)));
}

#[cfg(test)]
mod tests {
    use superfik_render::{Scene, TextSystem};

    use super::*;
    use crate::emote::EmoteState;
    use crate::icons;
    use crate::painter::LAYERS;

    fn paint(elapsed: f32, area: Rect) -> GetStartedHits {
        let mut scene = Scene::new(Color::TRANSPARENT, LAYERS);
        let mut text = TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &colors::LIGHT);
        let features = [
            Feature { icon: icons::LAYERS, tint: colors::LIGHT.skill, title: "One", body: "A line about it." },
            Feature { icon: icons::BRAIN, tint: colors::LIGHT.memory, title: "Two", body: "Another line." },
        ];
        let view = GetStarted {
            welcome: "Welcome aboard",
            emote: Emote { state: EmoteState::Idle, from: None, time: 0.0, width: 0.0 },
            title: "Get Started",
            features: &features,
            action: "Continue",
            action_hovered: false,
            elapsed,
        };
        view.paint(&mut p, area)
    }

    #[test]
    fn continue_is_offered_only_once_it_appears() {
        let area = Rect::new(0.0, 0.0, 1200.0, 800.0);
        assert_eq!(paint(0.0, area).action, None, "not during the intro");
        assert_eq!(paint(motion::GET_STARTED_LEAVE_AT, area).action, None);
        let action = paint(motion::GET_STARTED_OPEN, area).action.expect("offered at the end");
        assert!((action.center().x - 600.0).abs() < 0.01);
        assert_eq!((action.w, action.h), metrics::GET_STARTED_BUTTON);
        assert!(action.y > 400.0 && action.bottom() < 800.0, "{action:?}");
    }

    #[test]
    fn continue_is_clicked_where_it_lands_while_still_rising() {
        let area = Rect::new(0.0, 0.0, 1200.0, 800.0);
        let landing = paint(motion::GET_STARTED_OPEN, area).action;
        assert_eq!(paint(motion::GET_STARTED_OPEN - 0.1, area).action, landing);
    }
}
