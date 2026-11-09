//! The "open a pull request" sheet: grows out of the Repository node like
//! its pickers do, with the title and body fields, the draft toggle, "Write
//! with agent", and the buttons that confirm or cancel. Painted in window
//! coordinates over everything.

use superfik_i18n::Text;
use superfik_render::{GooShape, Point, Rect};
use superfik_ui::components::{Button, Chip, EarSide, NodeFrame, Sheet, Side, TextField};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, Plane, icons};

use super::motions::{Motions, SheetKind};
use super::target::{HitMap, Target};
use crate::state::{PrField, Session};

const WIDTH: f32 = 480.0;
const HEIGHT: f32 = 330.0;
const GAP: f32 = 8.0;
const STEM: f32 = 14.0;

#[derive(Debug, Clone, Copy)]
pub struct SheetAnchor {
    pub node: Rect,
    pub ear: Rect,
    pub stem_y: f32,
}

impl SheetAnchor {
    pub fn from_canvas(node: Rect, to_window: impl Fn(Rect) -> Rect) -> Self {
        let ear = NodeFrame::ear_rect(node, EarSide::Left);
        let stem_y = super::nodes::repository::pull_requests_row_centre(node);
        Self { node: to_window(node), ear: to_window(ear), stem_y: to_window(Rect::new(node.x, stem_y, 0.0, 0.0)).y }
    }
}

pub struct PullRequestSheetView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub caret_visible: bool,
    pub now: f64,
}

impl PullRequestSheetView<'_> {
    pub fn paint(
        &self,
        p: &mut Painter,
        motions: &mut Motions,
        from: Option<SheetAnchor>,
        hits: &mut HitMap,
    ) -> Option<Rect> {
        let open = self.session.pr_composer.is_some();
        motions.sync_sheet(SheetKind::PullRequest, open, self.now);
        let presence = motions.sheet(SheetKind::PullRequest, self.now)?;
        let composer = self.session.pr_composer.as_ref()?;
        let strings = self.session.strings();
        let viewport = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        let (anchor, side, panel, source) = match from {
            Some(from) => {
                let y = from.stem_y.clamp(HEIGHT * 0.5 + 8.0, viewport.bottom() - HEIGHT * 0.5 - 8.0);
                let x = from.ear.x - STEM - WIDTH;
                let source = GooShape::RoundRect { rect: from.node, radius: metrics::NODE_RADIUS };
                (
                    Point::new(from.node.x, from.stem_y),
                    Side::Left,
                    Rect::new(x, y - HEIGHT * 0.5, WIDTH, HEIGHT),
                    Some(source),
                )
            }
            None => {
                let c = viewport.center();
                (
                    Point::new(c.x, c.y + HEIGHT * 0.5 + STEM),
                    Side::Up,
                    Rect::new(c.x - WIDTH * 0.5, c.y - HEIGHT * 0.5, WIDTH, HEIGHT),
                    None,
                )
            }
        };
        let title = strings.t(Text::PullRequestSheetTitle);
        let sheet = Sheet {
            anchor,
            side,
            panel,
            source,
            progress: presence.progress(self.now),
            icon: icons::PULL_REQUEST,
            tint: Some(p.palette().branch),
            title: &title,
            close_hovered: self.hovered == Some(Target::CancelPullRequest),
        };
        let hovered = |target| self.hovered == Some(target);
        let mut fields = [Rect::default(); 2];
        let mut buttons: Vec<(Rect, Target)> = Vec::new();
        let painted = p.on_plane(Plane::Overlay, |p| {
            sheet.paint(p, |p, body| {
                let title_rect = Rect::new(body.x, body.y, body.w, metrics::ROW_HEIGHT);
                TextField {
                    edit: &composer.title,
                    placeholder: &strings.t(Text::PullRequestTitlePlaceholder),
                    focused: composer.focus == Some(PrField::Title),
                    caret_visible: self.caret_visible,
                    multiline: false,
                    secret: false,
                }
                .paint(p, title_rect);
                let footer = body.bottom() - metrics::CHIP_HEIGHT;
                let body_rect = Rect::new(
                    body.x,
                    title_rect.bottom() + metrics::ROW_GAP,
                    body.w,
                    footer - GAP - title_rect.bottom() - metrics::ROW_GAP,
                );
                TextField {
                    edit: &composer.body,
                    placeholder: &strings.t(Text::PullRequestBodyPlaceholder),
                    focused: composer.focus == Some(PrField::Body),
                    caret_visible: self.caret_visible,
                    multiline: true,
                    secret: false,
                }
                .paint(p, body_rect);
                fields = [title_rect, body_rect];

                let write = strings.t(Text::WriteWithAgent);
                let rect = Chip {
                    icon: icons::ROBOT,
                    label: &write,
                    active: composer.drafting,
                    hovered: hovered(Target::WritePullRequestWithAgent),
                    tint: None,
                }
                .paint(p, Point::new(body.x, footer));
                buttons.push((rect, Target::WritePullRequestWithAgent));
                let draft = strings.t(Text::DraftToggle);
                let rect = Chip {
                    icon: icons::PENCIL,
                    label: &draft,
                    active: composer.draft,
                    hovered: hovered(Target::TogglePullRequestDraft),
                    tint: None,
                }
                .paint(p, Point::new(rect.right() + GAP, footer));
                buttons.push((rect, Target::TogglePullRequestDraft));

                let create = strings.t(Text::CreatePullRequest);
                let create_button = Button {
                    label: &create,
                    primary: true,
                    hovered: hovered(Target::ConfirmPullRequest),
                    enabled: composer.can_confirm(),
                };
                let w = create_button.width(p);
                let rect = create_button.paint(p, Rect::new(body.right() - w, footer, w, metrics::CHIP_HEIGHT));
                buttons.push((rect, Target::ConfirmPullRequest));
                let cancel = strings.t(Text::Cancel);
                let cancel_button = Button {
                    label: &cancel,
                    primary: false,
                    hovered: hovered(Target::CancelPullRequest),
                    enabled: true,
                };
                let w = cancel_button.width(p);
                let rect = cancel_button.paint(p, Rect::new(rect.x - GAP - w, footer, w, metrics::CHIP_HEIGHT));
                buttons.push((rect, Target::CancelPullRequest));
            })
        });
        if presence.is_visible() && painted.content_opacity > 0.6 {
            hits.add_screen(painted.panel, Target::Surface);
            hits.add_screen(painted.close, Target::CancelPullRequest);
            hits.add_screen(fields[0], Target::PullRequestField(PrField::Title));
            hits.add_screen(fields[1], Target::PullRequestField(PrField::Body));
            for (rect, target) in buttons {
                hits.add_screen(rect, target);
            }
        }
        presence.is_visible().then_some(panel)
    }
}

#[cfg(test)]
mod tests {
    use superfik_render::{Point, Scene};
    use superfik_ui::painter::LAYERS;

    use super::*;
    use crate::state::Action;

    fn paint(session: &Session, motions: &mut Motions, now: f64) -> HitMap {
        let mut scene = Scene::new(superfik_render::Color::TRANSPARENT, LAYERS);
        let mut text = superfik_render::TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &superfik_ui::theme::colors::DARK);
        let mut hits = HitMap::default();
        let view = PullRequestSheetView { session, hovered: None, viewport: (1400.0, 900.0), caret_visible: true, now };
        let from = SheetAnchor::from_canvas(Rect::new(700.0, 300.0, 216.0, 200.0), |r| r);
        view.paint(&mut p, motions, Some(from), &mut hits);
        hits
    }

    fn at(hits: &HitMap, target: Target) -> Option<Rect> {
        (0..140)
            .flat_map(|x| (0..90).map(move |y| Point::new(x as f32 * 10.0, y as f32 * 10.0)))
            .find_map(|point| (hits.at(point, point) == Some(target)).then(|| Rect::new(point.x, point.y, 1.0, 1.0)))
    }

    #[test]
    fn the_open_sheet_offers_cancel_and_confirm_once_its_content_has_resolved() {
        let mut session = Session::new(Some("/r".into()));
        session.snapshot.repository = Some(superfik_core::Repository { root: "/r".into(), ..Default::default() });
        session.apply(Action::ProposePullRequest, 0.0);
        let mut motions = Motions::default();
        let early = paint(&session, &mut motions, 0.0);
        assert_eq!(at(&early, Target::CancelPullRequest), None, "still growing");
        let open = paint(&session, &mut motions, 3.0);
        assert!(at(&open, Target::CancelPullRequest).is_some());
        assert!(at(&open, Target::ConfirmPullRequest).is_some());
        assert!(at(&open, Target::PullRequestField(PrField::Body)).is_some());
        session.apply(Action::CancelPullRequest, 3.0);
        let closed = paint(&session, &mut motions, 3.1);
        assert_eq!(at(&closed, Target::CancelPullRequest), None);
    }
}
