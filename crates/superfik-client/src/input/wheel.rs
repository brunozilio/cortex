//! Where the wheel or two fingers go, for every shell: a screen on top
//! scrolls (and the canvas beneath stays still), then the panes and lists
//! under the pointer, paged nodes, the sidebar, the composer's text, and
//! last the canvas itself. Pure; the shell owns the camera, the sidebar's
//! scroll, the graph's view and the detent counters.

use superfik_render::{Point, Rect};

use crate::state::{Action, Carousel, RoutineAction, Session};
use crate::view::target::{HitMap, Target};

pub const WHEEL_ZOOM: f32 = 0.01;

fn act(action: Action) -> Wheel {
    Wheel::Act(Box::new(action))
}

#[derive(Debug, Clone)]
pub enum Wheel {
    Act(Box<Action>),
    Graph { graph: Rect, zoom: Option<f32> },
    Flip(Carousel),
    Sidebar,
    InputText,
    Zoom(f32),
    Pan,
    Still,
}

pub fn route_wheel(
    session: &Session,
    hits: &HitMap,
    at: Point,
    canvas_at: Point,
    delta: (f32, f32),
    zooming: bool,
    hovered: Option<Target>,
) -> Wheel {
    let (dx, dy) = delta;
    if session.routines.open {
        return hits
            .routines_scroll
            .map_or(Wheel::Still, |(_, limit)| act(Action::Routines(RoutineAction::Scroll { delta: -dy, limit })));
    }
    if session.settings.open {
        return hits.settings_scroll.map_or(Wheel::Still, |(_, max)| act(Action::ScrollSettings { by: -dy, max }));
    }
    if hits.expanded_page.is_some() {
        return Wheel::Still;
    }
    if let Some((_, max)) = hits
        .model_scroll
        .filter(|(panel, _)| session.popover == Some(crate::state::PopoverKind::Model) && panel.contains(canvas_at))
    {
        return act(Action::ScrollModels { by: -dy, max });
    }
    if let Some((files, diff)) = hits.review_panes.filter(|_| session.review_open()) {
        let over_diff = diff.contains(at);
        let files = if files.contains(at) { -dy } else { 0.0 };
        let (diff, diff_x) = if over_diff { (-dy, -dx) } else { (0.0, 0.0) };
        if files == 0.0 && diff == 0.0 && diff_x == 0.0 {
            return Wheel::Still;
        }
        return act(Action::ScrollReview { files, diff, diff_x });
    }
    if let Some(graph) = hits.memory_graph.filter(|graph| session.memory.open && graph.contains(at)) {
        return Wheel::Graph { graph, zoom: zooming.then(|| (dy * WHEEL_ZOOM).exp()) };
    }
    if let Some((_, max)) = hits.agent_sheet.filter(|(body, _)| body.contains(at)) {
        return act(Action::ScrollAgent { by: dy, max });
    }
    if let Some((list, max)) = hits.memory_list.filter(|_| session.memory.open) {
        return if list.contains(at) { act(Action::ScrollMemory { by: -dy, max }) } else { Wheel::Still };
    }
    if let Some(carousel) = carousel_under(hits, at, canvas_at) {
        return Wheel::Flip(carousel);
    }
    if hits.on_screen(at) {
        return Wheel::Sidebar;
    }
    if hovered == Some(Target::Input) && hits.input_scroll.is_some_and(|(_, over)| over) {
        return Wheel::InputText;
    }
    if zooming {
        return Wheel::Zoom((dy * WHEEL_ZOOM).exp());
    }
    Wheel::Pan
}

pub fn carousel_under(hits: &HitMap, at: Point, canvas_at: Point) -> Option<Carousel> {
    if hits.on_screen(at) {
        return None;
    }
    hits.carousels.iter().rev().find(|(node, _)| node.contains(canvas_at)).map(|(_, carousel)| *carousel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(session: &Session, hits: &HitMap, zooming: bool) -> Wheel {
        let at = Point::new(500.0, 400.0);
        route_wheel(session, hits, at, at, (0.0, 30.0), zooming, None)
    }

    #[test]
    fn the_canvas_pans_and_zooms_when_nothing_else_is_under_the_pointer() {
        let s = Session::default();
        assert!(matches!(wheel(&s, &HitMap::default(), false), Wheel::Pan));
        assert!(matches!(wheel(&s, &HitMap::default(), true), Wheel::Zoom(f) if f > 1.0));
    }

    #[test]
    fn a_screen_on_top_scrolls_and_the_canvas_stays_still() {
        let mut s = Session::default();
        s.apply(Action::OpenSettings, 0.0);
        assert!(matches!(wheel(&s, &HitMap::default(), false), Wheel::Still), "nothing to scroll yet");
        let mut hits = HitMap::default();
        hits.settings_scroll = Some((Rect::new(0.0, 0.0, 10.0, 10.0), 200.0));
        let Wheel::Act(action) = wheel(&s, &hits, false) else { unreachable!("the pane scrolls") };
        assert!(matches!(*action, Action::ScrollSettings { by, max } if by == -30.0 && max == 200.0));
    }

    #[test]
    fn an_expanded_page_keeps_everything_beneath_still() {
        let mut hits = HitMap::default();
        hits.expanded_page = Some((superfik_core::BrowserId::new(), Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(matches!(wheel(&Session::default(), &hits, false), Wheel::Still));
    }
}
