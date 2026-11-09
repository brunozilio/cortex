//! Browser node: a web page on the canvas. While the card is expanded (and
//! while it grows to the window or shrinks back) the page shows over the
//! window instead (`view::browser_overlay`) and the node keeps only its
//! frame.

use superfik_i18n::{Strings, Text};
use superfik_render::Rect;
use superfik_ui::Painter;
use superfik_ui::components::{BrowserCard, BrowserPart, PageImage};

use super::NodeCtx;
use crate::state::{BrowserCard as Card, BrowserFocus};
use crate::view::layout::NodeKey;
use crate::view::target::Target;

pub fn height() -> f32 {
    BrowserCard::height()
}

pub struct Labels {
    name: String,
    placeholder: String,
    agent: String,
}

impl Labels {
    pub fn new(strings: &Strings) -> Self {
        Self {
            name: strings.t(Text::Browser),
            placeholder: strings.t(Text::BrowserAddress),
            agent: strings.t(Text::BrowserAgent),
        }
    }
}

pub fn props<'a>(
    card: &'a Card,
    labels: &'a Labels,
    rect: Rect,
    page: Option<PageImage>,
    hovered: Option<Target>,
    caret_visible: bool,
) -> BrowserCard<'a> {
    let hovered = match hovered {
        Some(Target::Browser(part)) => Some(part),
        _ => None,
    };
    let site = superfik_browser::address::site(&card.url);
    BrowserCard {
        rect,
        title: if card.title.is_empty() && site.is_empty() { &labels.name } else { &card.title },
        site,
        address: &card.address,
        placeholder: &labels.placeholder,
        editing: card.focus == Some(BrowserFocus::Address),
        caret_visible,
        loading: card.loading || card.driven(),
        back: card.back,
        forward: card.forward,
        private: card.private,
        logins: !card.logins().is_empty() || card.offer.is_some() || !card.passkeys().is_empty(),
        logins_open: false,
        expanded: false,
        turn: 0.0,
        page,
        failure: card.failure.as_deref(),
        hovered,
        agent: card.agent.then_some(labels.agent.as_str()),
        ear_hovered: false,
        content: 1.0,
        time: 0.0,
    }
}

pub fn paint(
    p: &mut Painter,
    ctx: &mut NodeCtx,
    card: &Card,
    page: Option<PageImage>,
    caret_visible: bool,
    overlaid: bool,
    logins_open: bool,
) {
    let labels = Labels::new(&ctx.strings);
    let shown = page.filter(|_| !overlaid);
    let mut props = props(card, &labels, ctx.rect, shown, ctx.hovered, caret_visible);
    props.logins_open = logins_open;
    props.ear_hovered = ctx.is_hovered(Target::Close(NodeKey::Browser));
    props.content = ctx.content;
    props.time = ctx.time;
    let hits = props.paint(p);
    for (rect, part) in hits.parts {
        if overlaid && part == BrowserPart::Page {
            continue;
        }
        ctx.hits.add(rect, Target::Browser(part));
    }
    if let Some(knob) = hits.knob {
        ctx.hits.add(knob, Target::Close(NodeKey::Browser));
    }
    ctx.hits.browser_card = Some(ctx.rect);
    if ctx.content >= 1.0 && !overlaid {
        ctx.hits.browser_page = Some((card.id, hits.page));
    }
}
