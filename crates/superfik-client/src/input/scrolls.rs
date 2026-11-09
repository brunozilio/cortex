//! What a scroll moves besides the camera, kept from frame to frame for
//! every shell: detents of the sidebar's sessions and of paged nodes, the
//! explorer graph's view, and the composer text's scroll. Pure; the shell
//! dispatches the actions and moves its camera.

use superfik_render::Point;
use superfik_ui::components::GraphView;
use superfik_ui::theme::metrics;

use super::wheel::Wheel;
use super::wheel_steps::WheelSteps;
use crate::state::{Action, Carousel, Toward};
use crate::view::target::HitMap;

#[derive(Debug, Default)]
pub struct Scrolls {
    sidebar: WheelSteps,
    pages: WheelSteps,
    pages_over: Option<Carousel>,
    pub graph: GraphView,
    pub input: InputScroll,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputScroll {
    pub offset: f32,
    pub follows_caret: bool,
}

impl Default for InputScroll {
    fn default() -> Self {
        Self { offset: 0.0, follows_caret: true }
    }
}

#[derive(Debug, Default)]
pub struct Scrolled {
    pub actions: Vec<Action>,
    pub zoom: Option<f32>,
    pub pan: Option<(f32, f32)>,
    pub stepped_session: bool,
}

impl Scrolls {
    pub fn apply(&mut self, wheel: Wheel, (dx, dy): (f32, f32), at: Point) -> Scrolled {
        let mut done = Scrolled::default();
        let flipping = matches!(wheel, Wheel::Flip(_));
        match wheel {
            Wheel::Act(action) => done.actions.push(*action),
            Wheel::Graph { graph, zoom: Some(factor) } => self.graph.zoom_at(factor, at, graph),
            Wheel::Graph { graph, zoom: None } => self.graph.pan_by(dx, dy, graph),
            Wheel::Flip(carousel) => done.actions.extend(self.flip(carousel, dx, dy)),
            Wheel::Sidebar => {
                if let Some((toward, rows)) = self.sidebar.feed(dy, metrics::TIMELINE_ROW) {
                    done.actions.extend((0..rows).map(|_| Action::ScrollConversation(toward)));
                    done.stepped_session = true;
                }
                return done;
            }
            Wheel::InputText => self.input = InputScroll { offset: self.input.offset - dy, follows_caret: false },
            Wheel::Zoom(factor) => done.zoom = Some(factor),
            Wheel::Pan => done.pan = Some((dx, dy)),
            Wheel::Still => {}
        }
        self.sidebar.reset();
        if !flipping {
            self.pages.reset();
        }
        done
    }

    fn flip(&mut self, carousel: Carousel, dx: f32, dy: f32) -> Option<Action> {
        if self.pages_over != Some(carousel) {
            self.pages.reset();
            self.pages_over = Some(carousel);
        }
        let travel = if dx.abs() > dy.abs() { dx } else { dy };
        let (toward, pages) = self.pages.feed(travel, metrics::PAGE_FLIP)?;
        let pages = pages as i32;
        Some(Action::Page(carousel, if toward == Toward::Older { pages } else { -pages }))
    }

    pub fn edited(&mut self) {
        self.input.follows_caret = true;
    }

    pub fn painted(&mut self, hits: &HitMap, explorer_open: bool) {
        if let Some((offset, _)) = hits.input_scroll {
            self.input.offset = offset;
        }
        if !explorer_open {
            self.graph = GraphView::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_render::Rect;

    use super::*;

    const AT: Point = Point::new(10.0, 10.0);

    #[test]
    fn the_sidebar_steps_a_session_per_row_of_travel() {
        let mut scrolls = Scrolls::default();
        assert!(scrolls.apply(Wheel::Sidebar, (0.0, -metrics::TIMELINE_ROW * 0.6), AT).actions.is_empty());
        let done = scrolls.apply(Wheel::Sidebar, (0.0, -metrics::TIMELINE_ROW * 0.6), AT);
        assert!(matches!(done.actions.as_slice(), [Action::ScrollConversation(Toward::Older)]));
        assert!(done.stepped_session);
    }

    #[test]
    fn a_paged_node_flips_a_page_per_detent() {
        let mut scrolls = Scrolls::default();
        let carousel = Carousel::Skills;
        let done = scrolls.apply(Wheel::Flip(carousel), (0.0, -metrics::PAGE_FLIP * 2.2), AT);
        assert!(matches!(done.actions.as_slice(), [Action::Page(c, 2)] if *c == carousel));
    }

    #[test]
    fn the_graph_pans_and_zooms_and_is_forgotten_when_closed() {
        let mut scrolls = Scrolls::default();
        let graph = Rect::new(0.0, 0.0, 400.0, 400.0);
        scrolls.apply(Wheel::Graph { graph, zoom: Some(1.5) }, (0.0, 0.0), Point::new(200.0, 200.0));
        assert!(scrolls.graph.zoom > 1.0);
        scrolls.painted(&HitMap::default(), false);
        assert_eq!(scrolls.graph, GraphView::default());
    }

    #[test]
    fn the_text_stops_following_the_caret_until_the_next_edit() {
        let mut scrolls = Scrolls::default();
        scrolls.apply(Wheel::InputText, (0.0, -30.0), AT);
        assert_eq!(scrolls.input, InputScroll { offset: 30.0, follows_caret: false });
        scrolls.edited();
        assert!(scrolls.input.follows_caret);
    }

    #[test]
    fn the_camera_is_the_shells() {
        let mut scrolls = Scrolls::default();
        assert_eq!(scrolls.apply(Wheel::Pan, (3.0, 4.0), AT).pan, Some((3.0, 4.0)));
        assert_eq!(scrolls.apply(Wheel::Zoom(1.1), (0.0, 0.0), AT).zoom, Some(1.1));
    }
}
