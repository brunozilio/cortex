//! What a press and a drag do, once `Pointer::press` has said what kind
//! of press it was: the caret or a word in the composer, the effort meter,
//! a session's title, a click. Pure; window moves and the camera are the
//! shell's.

use superfik_render::Point;
use superfik_ui::components::text_hit;
use superfik_ui::theme::typography;

use super::controller;
use super::pointer::{Pointer, Press, effort_at};
use crate::state::{Action, EditKey, Session};
use crate::view::target::{HitMap, ReviewTarget, Target};

#[derive(Debug, Clone)]
pub enum Pressed {
    Act(Vec<Action>),
    Recentre,
    Window(Press),
}

pub fn pressed(
    session: &Session,
    hits: &HitMap,
    press: Press,
    target: Option<Target>,
    at: Point,
    shift: bool,
) -> Pressed {
    match press {
        Press::DragWindow | Press::Maximize | Press::Minimize => Pressed::Window(press),
        Press::Effort => Pressed::Act(slide(session, hits, at).into_iter().collect()),
        Press::Rename(id) => Pressed::Act(vec![Action::BeginRename(id)]),
        Press::Text { double } => {
            let mut actions = controller::click(Some(Target::Input), session);
            if let Some(offset) = text_offset(hits, at) {
                let key = if double { EditKey::SelectWord(offset) } else { EditKey::MoveTo { offset, select: shift } };
                actions.push(Action::Edit(key));
            }
            Pressed::Act(actions)
        }
        Press::Click => match target {
            Some(Target::RecentreComposer) => Pressed::Recentre,
            Some(Target::Review(ReviewTarget::Line { side, line })) if shift => {
                Pressed::Act(vec![Action::SelectDiffLine { side, line, extend: true }])
            }
            target => Pressed::Act(controller::click(target, session)),
        },
    }
}

pub fn slide(session: &Session, hits: &HitMap, at: Point) -> Option<Action> {
    let effort = effort_at(hits.effort_track?, at.x);
    (effort != session.effort()).then_some(Action::SetEffort(effort))
}

pub fn dragged(session: &Session, hits: &HitMap, pointer: &Pointer, at: Point) -> Vec<Action> {
    let mut actions = Vec::new();
    if pointer.sliding_effort {
        actions.extend(slide(session, hits, at));
    }
    if pointer.selecting_text
        && let Some(offset) = text_offset(hits, at)
    {
        actions.push(Action::Edit(EditKey::MoveTo { offset, select: true }));
    }
    actions
}

pub fn text_offset(hits: &HitMap, at: Point) -> Option<usize> {
    let (layout, origin) = hits.input_text.as_ref()?;
    Some(text_hit::offset_at(layout, at.x - origin.x, at.y - origin.y, typography::INPUT.line_height))
}

#[cfg(test)]
mod tests {
    use superfik_core::ConversationId;
    use superfik_render::Rect;

    use super::*;

    #[test]
    fn the_title_bar_belongs_to_the_window() {
        let s = Session::default();
        let pressed = pressed(&s, &HitMap::default(), Press::DragWindow, None, Point::default(), false);
        assert!(matches!(pressed, Pressed::Window(Press::DragWindow)));
    }

    #[test]
    fn a_double_click_on_a_session_renames_it() {
        let s = Session::default();
        let id = ConversationId::new();
        let pressed = pressed(&s, &HitMap::default(), Press::Rename(id), None, Point::default(), false);
        assert!(
            matches!(pressed.clone(), Pressed::Act(ref a) if matches!(a.as_slice(), [Action::BeginRename(r)] if *r == id))
        );
    }

    #[test]
    fn the_recentre_knob_moves_the_camera_not_the_session() {
        let s = Session::default();
        let target = Some(Target::RecentreComposer);
        assert!(matches!(
            pressed(&s, &HitMap::default(), Press::Click, target, Point::default(), false),
            Pressed::Recentre
        ));
    }

    #[test]
    fn the_effort_meter_follows_the_pointer_only_to_another_level() {
        let s = Session::default();
        let mut hits = HitMap::default();
        hits.effort_track = Some(Rect::new(0.0, 0.0, 100.0, 10.0));
        let far = slide(&s, &hits, Point::new(100.0, 5.0));
        assert!(matches!(far, Some(Action::SetEffort(e)) if e != s.effort()));
        let here = (0..=100).map(|x| Point::new(x as f32, 5.0)).find(|at| slide(&s, &hits, *at).is_none());
        assert!(here.is_some(), "the current level slides nowhere");
        let mut pointer = Pointer::new(super::super::titlebar::DoubleClickAction::None);
        assert!(dragged(&s, &hits, &pointer, Point::new(100.0, 5.0)).is_empty(), "not sliding");
        pointer.sliding_effort = true;
        assert_eq!(dragged(&s, &hits, &pointer, Point::new(100.0, 5.0)).len(), 1);
    }
}
