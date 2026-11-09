//! Pull request node: the one the conversation works on, hanging off the
//! Repository node (composer → repository → pull request). Its card opens
//! the review screen; the ear detaches it (the branch stays).

use superfik_core::PullRequest;
use superfik_i18n::Text;
use superfik_render::{Point, Quad, Rect};
use superfik_ui::components::{EarSide, NodeFrame};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, reveal};
use crate::view::layout::NodeKey;
use crate::view::target::Target;

const CARD: f32 = 52.0;

pub fn height() -> f32 {
    node_height(CARD + metrics::ROW_GAP + metrics::ROW_HEIGHT)
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, pr: &PullRequest, pending: usize) {
    let strings = ctx.strings;
    let title = strings.t(Text::PullRequest);
    let count = pending.to_string();
    let mut frame = NodeFrame::new(ctx.rect, icons::PULL_REQUEST, Some(p.palette().branch), &title);
    frame.count = (pending > 0).then_some(count.as_str());
    frame.busy = ctx.busy;
    frame.ear = Some(EarSide::Left);
    frame.ear_hovered = ctx.is_hovered(Target::Close(NodeKey::PullRequest));
    frame.content = ctx.content;
    frame.time = ctx.time;
    let hits = frame.paint(p);
    if let Some(knob) = hits.knob {
        ctx.hits.add(knob, Target::Close(NodeKey::PullRequest));
    }
    let body = ctx.body();
    let card = Rect::new(body.x, body.y, body.w, CARD);
    reveal(p, ctx.content, |p| {
        let fill = if ctx.is_hovered(Target::OpenReview) { p.palette().hover } else { p.palette().well };
        p.quad(Quad::new(card, metrics::WELL_RADIUS, fill));
        let heading = format!("#{} {}", pr.number, pr.title);
        let subtitle = if pr.author.is_empty() {
            pr.branches_caption()
        } else {
            format!("{} · {}", pr.author, pr.branches_caption())
        };
        let (x, max) = (card.x + 12.0, card.w - 24.0);
        p.label_fit(&heading, typography::TITLE, x, card.y + 17.0, max, p.palette().text);
        p.label_fit(&subtitle, typography::SUBTITLE, x, card.y + 38.5, max, p.palette().text_subtle);
        ctx.hits.add(card, Target::OpenReview);
        let delegate = Rect::new(body.x, card.bottom() + metrics::ROW_GAP, body.w, metrics::ROW_HEIGHT);
        let hovered = ctx.is_hovered(Target::DelegateReview);
        let color = if hovered { p.palette().text } else { p.palette().text_subtle };
        let glyph = if hovered { p.palette().text } else { p.palette().knob };
        p.icon_at(icons::ROBOT, Point::new(delegate.x + 13.0, delegate.center().y - 1.0), 13.0, Some(glyph));
        p.label(&strings.t(Text::ReviewWithAgent), typography::SUBTITLE, delegate.x + 25.0, delegate.center().y, color);
        ctx.hits.add(delegate, Target::DelegateReview);
    });
}
