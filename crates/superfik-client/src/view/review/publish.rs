//! The publish sheet: grows down out of the header's verdict buttons with
//! the summary, the drafts going out (each one toggleable) and the button
//! that submits the review.

use superfik_i18n::{Strings, Text};
use superfik_render::{GooShape, Point, Rect};
use superfik_ui::components::{Button, Row, Sheet, Side, TextField, Trailing};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, Plane, icons};

use super::header;
use crate::state::{ReviewScreen, Session};
use crate::view::motions::{Motions, SheetKind};
use crate::view::target::{HitMap, ReviewTarget, Target};

const WIDTH: f32 = 460.0;
const HEIGHT: f32 = 360.0;
const SUMMARY: f32 = 66.0;
const GAP: f32 = 8.0;
const STEM: f32 = 14.0;

#[allow(clippy::too_many_arguments, reason = "a view entry point: everything it paints from, nothing stored")]
pub fn paint(
    p: &mut Painter,
    session: &Session,
    review: &ReviewScreen,
    strings: &Strings,
    area: Rect,
    motions: &mut Motions,
    now: f64,
    hovered: Option<Target>,
    caret_visible: bool,
    hits: &mut HitMap,
) {
    motions.sync_sheet(SheetKind::PublishReview, review.publish.is_some(), now);
    let Some(presence) = motions.sheet(SheetKind::PublishReview, now) else { return };
    let Some(publish) = review.publish.as_ref() else { return };
    let is = |t: ReviewTarget| hovered == Some(Target::Review(t));
    let title = format!("{} · {}", strings.t(Text::ReviewPublishSheetTitle), verdict_label(strings, publish.verdict));
    let header_rect = Rect::new(area.x, area.y + session.platform.titlebar_height(), area.w, header::HEIGHT);
    let anchor = Point::new(area.right() - 110.0, header_rect.bottom() - 12.0);
    let panel = Rect::new(area.right() - 12.0 - WIDTH, header_rect.bottom() + STEM, WIDTH, HEIGHT);
    let sheet = Sheet {
        anchor,
        side: Side::Down,
        panel,
        source: Some(GooShape::RoundRect {
            rect: Rect::new(anchor.x - 60.0, anchor.y - 20.0, 120.0, 32.0),
            radius: metrics::CHIP_RADIUS,
        }),
        progress: presence.progress(now),
        icon: icons::CHECK_CIRCLE,
        tint: Some(p.palette().branch),
        title: &title,
        close_hovered: is(ReviewTarget::CancelPublish),
    };
    let mut rows: Vec<(Rect, Target)> = Vec::new();
    let painted = p.on_plane(Plane::Overlay, |p| {
        sheet.paint(p, |p, body| {
            let summary = Rect::new(body.x, body.y, body.w, SUMMARY);
            TextField {
                edit: &publish.summary,
                placeholder: &strings.t(Text::ReviewSummaryPlaceholder),
                focused: true,
                caret_visible,
                multiline: true,
                secret: false,
            }
            .paint(p, summary);
            let footer = body.bottom() - metrics::CHIP_HEIGHT;
            let list = Rect::new(
                body.x,
                summary.bottom() + metrics::ROW_GAP,
                body.w,
                footer - GAP - summary.bottom() - metrics::ROW_GAP,
            );
            p.clipped(list, |p| {
                let mut y = list.y;
                for draft in &review.drafts {
                    let row = Rect::new(list.x, y, list.w, metrics::ROW_HEIGHT);
                    let included = !publish.skipped.contains(&draft.id);
                    let label =
                        format!("{}:{} — {}", draft.path, draft.line, draft.body.lines().next().unwrap_or_default());
                    let trailing = if included { Trailing::Check } else { Trailing::None };
                    Row::new(&label)
                        .icon(icons::CHAT, Some(if included { p.palette().icon } else { p.palette().icon_muted }))
                        .trailing(trailing)
                        .color(if included { p.palette().text_label } else { p.palette().text_secondary })
                        .hovered(is(ReviewTarget::Outgoing(draft.id)))
                        .paint(p, row);
                    rows.push((row.intersect(&list), Target::Review(ReviewTarget::Outgoing(draft.id))));
                    y += metrics::ROW_HEIGHT + metrics::ROW_GAP;
                }
                if review.drafts.is_empty() {
                    p.label(
                        &strings.t(Text::ReviewNoFiles),
                        typography::SUBTITLE,
                        list.x + 8.0,
                        list.y + 14.0,
                        p.palette().text_subtle,
                    );
                }
            });
            let publish_label = strings.t(Text::ReviewPublish);
            let publish_button = Button {
                label: &publish_label,
                primary: true,
                hovered: is(ReviewTarget::Publish),
                enabled: !review.publishing,
            };
            let w = publish_button.width(p);
            let rect = publish_button.paint(p, Rect::new(body.right() - w, footer, w, metrics::CHIP_HEIGHT));
            rows.push((rect, Target::Review(ReviewTarget::Publish)));
            let cancel_label = strings.t(Text::Cancel);
            let cancel = Button {
                label: &cancel_label,
                primary: false,
                hovered: is(ReviewTarget::CancelPublish),
                enabled: true,
            };
            let w = cancel.width(p);
            let rect = cancel.paint(p, Rect::new(rect.x - GAP - w, footer, w, metrics::CHIP_HEIGHT));
            rows.push((rect, Target::Review(ReviewTarget::CancelPublish)));
        })
    });
    if presence.is_visible() && painted.content_opacity > 0.6 {
        hits.add_screen(painted.panel, Target::Review(ReviewTarget::Surface));
        hits.add_screen(painted.close, Target::Review(ReviewTarget::CancelPublish));
        for (rect, target) in rows {
            hits.add_screen(rect, target);
        }
    }
}

fn verdict_label(strings: &Strings, verdict: superfik_core::ReviewVerdict) -> String {
    match verdict {
        superfik_core::ReviewVerdict::Approve => strings.t(Text::ReviewApprove),
        superfik_core::ReviewVerdict::RequestChanges => strings.t(Text::ReviewRequestChanges),
        superfik_core::ReviewVerdict::Comment => strings.t(Text::ReviewAddComment),
    }
}
