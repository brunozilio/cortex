//! The review screen's header: back, the pull request, layout toggle and
//! the actions (comment, ask agent, approve, request changes).

use superfik_i18n::{Strings, Text};
use superfik_render::{Point, Rect};
use superfik_ui::Painter;
use superfik_ui::components::{Button, ScreenHeader, Segmented};
use superfik_ui::theme::{metrics, typography};

use crate::state::{DiffLayout, ReviewScreen, Session};
use crate::view::target::{HitMap, ReviewTarget, Target};

pub const HEIGHT: f32 = metrics::SCREEN_HEADER;
const GAP: f32 = metrics::SCREEN_ACTION_GAP;

pub fn paint(
    p: &mut Painter,
    session: &Session,
    review: &ReviewScreen,
    strings: &Strings,
    area: Rect,
    hovered: Option<Target>,
    hits: &mut HitMap,
) -> Rect {
    let is = |t: ReviewTarget| hovered == Some(Target::Review(t));
    let back = strings.t(Text::ReviewBack);
    let layout = ScreenHeader {
        top_inset: session.platform.titlebar_height(),
        back: &back,
        back_hovered: is(ReviewTarget::Back),
    }
    .paint(p, area);
    let (header, cy) = (layout.bar, layout.center_y);
    hits.add_screen(layout.back, Target::Review(ReviewTarget::Back));
    let pr = session.conversation().pull_request.as_ref();
    let title = pr.map(|pr| format!("#{} {}", pr.number, pr.title)).unwrap_or_else(|| format!("#{}", review.number));
    let mut x = layout.actions_right;
    let mut button =
        |p: &mut Painter, label: String, primary: bool, enabled: bool, target: ReviewTarget, x: &mut f32| {
            let b = Button { label: &label, primary, hovered: is(target), enabled };
            let w = b.width(p);
            let rect = b.paint(p, Rect::new(*x - w, cy - metrics::CHIP_HEIGHT * 0.5, w, metrics::CHIP_HEIGHT));
            hits.add_screen(rect, Target::Review(target));
            *x = rect.x - GAP;
        };
    let loaded = review.detail.is_some();
    button(p, strings.t(Text::ReviewApprove), true, loaded && !review.publishing, ReviewTarget::Approve, &mut x);
    button(
        p,
        strings.t(Text::ReviewRequestChanges),
        false,
        loaded && !review.publishing,
        ReviewTarget::RequestChanges,
        &mut x,
    );
    let has_selection = review.selection.is_some();
    button(p, strings.t(Text::ReviewAskAgent), false, has_selection, ReviewTarget::AskAgent, &mut x);
    button(p, strings.t(Text::ReviewAddComment), false, has_selection, ReviewTarget::AddComment, &mut x);
    let labels = [strings.t(Text::ReviewStacked), strings.t(Text::ReviewSplit)];
    let labels_ref = [labels[0].as_str(), labels[1].as_str()];
    let selected = match review.layout {
        DiffLayout::Stacked => 0,
        DiffLayout::Split => 1,
    };
    let seg_hovered = [DiffLayout::Stacked, DiffLayout::Split].iter().position(|l| is(ReviewTarget::Layout(*l)));
    let segmented = Segmented { labels: &labels_ref, selected, hovered: seg_hovered };
    let widths: f32 = labels_ref.iter().map(|l| p.layout(l, typography::CHIP, None).width + 22.0).sum::<f32>() + 4.0;
    let rects = segmented.paint(p, Point::new(x - widths, cy - Segmented::HEIGHT * 0.5));
    for (rect, layout) in rects.into_iter().zip([DiffLayout::Stacked, DiffLayout::Split]) {
        hits.add_screen(rect, Target::Review(ReviewTarget::Layout(layout)));
    }
    ScreenHeader::title(p, &layout, &title, x - widths);
    header
}
