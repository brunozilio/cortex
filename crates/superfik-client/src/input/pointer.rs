//! What a mouse press means before any click action runs: moving the
//! window, the title bar double-click, or a plain click. Pure and
//! tested; the shell performs the window calls.

use superfik_core::Effort;
use superfik_render::{Point, Rect};
use superfik_ui::components::ComposerPart;

use super::titlebar::{self, ClickCounter, DoubleClickAction};
use crate::view::target::{
    GetStartedTarget, LockTarget, MemoryTarget, ReviewTarget, RoutinesTarget, SettingsTarget, Target, UpdateTarget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    DragWindow,
    Maximize,
    Minimize,
    Effort,
    Rename(superfik_core::ConversationId),
    Text { double: bool },
    Click,
}

#[derive(Debug)]
pub struct Pointer {
    pub position: Point,
    pub sliding_effort: bool,
    pub selecting_text: bool,
    clicks: ClickCounter,
    row_clicks: ClickCounter,
    text_clicks: ClickCounter,
    double_click: DoubleClickAction,
    titlebar_height: f32,
}

impl Pointer {
    pub fn new(double_click: DoubleClickAction) -> Self {
        Self {
            position: Point::default(),
            sliding_effort: false,
            selecting_text: false,
            text_clicks: ClickCounter::default(),
            clicks: ClickCounter::default(),
            row_clicks: ClickCounter::default(),
            double_click,
            titlebar_height: titlebar::HEIGHT,
        }
    }

    pub fn with_titlebar(mut self, height: f32) -> Self {
        self.titlebar_height = height;
        self
    }

    pub fn press(&mut self, target: Option<Target>, time: f64) -> Press {
        match target {
            None
            | Some(
                Target::Surface
                | Target::Settings(SettingsTarget::Surface)
                | Target::Routines(RoutinesTarget::Surface)
                | Target::Review(ReviewTarget::Surface)
                | Target::Memory(MemoryTarget::Surface)
                | Target::Update(UpdateTarget::Surface)
                | Target::Lock(LockTarget::Surface)
                | Target::GetStarted(GetStartedTarget::Surface),
            ) if self.position.y >= 0.0 && self.position.y < self.titlebar_height => {
                if !self.clicks.press(time, self.position) {
                    return Press::DragWindow;
                }
                match self.double_click {
                    DoubleClickAction::Maximize => Press::Maximize,
                    DoubleClickAction::Minimize => Press::Minimize,
                    DoubleClickAction::None => Press::Click,
                }
            }
            Some(Target::Composer(ComposerPart::Effort)) => {
                self.sliding_effort = true;
                Press::Effort
            }
            Some(Target::Conversation(id)) => {
                if self.row_clicks.press(time, self.position) {
                    Press::Rename(id)
                } else {
                    Press::Click
                }
            }
            Some(Target::Input) => {
                self.selecting_text = true;
                Press::Text { double: self.text_clicks.press(time, self.position) }
            }
            _ => Press::Click,
        }
    }
}

pub fn effort_at(track: Rect, x: f32) -> Effort {
    Effort::from_level((x - track.x) / track.w.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::layout::NodeKey;

    fn pointer_at(x: f32, y: f32) -> Pointer {
        let mut pointer = Pointer::new(DoubleClickAction::Maximize);
        pointer.position = Point::new(x, y);
        pointer
    }

    #[test]
    fn pressing_the_text_starts_a_selection_and_twice_picks_a_word() {
        let mut pointer = pointer_at(200.0, 300.0);
        assert_eq!(pointer.press(Some(Target::Input), 1.0), Press::Text { double: false });
        assert!(pointer.selecting_text);
        assert_eq!(pointer.press(Some(Target::Input), 1.1), Press::Text { double: true });
    }

    #[test]
    fn title_bar_drags_then_maximizes_on_double_click() {
        let mut pointer = pointer_at(400.0, 10.0);
        assert_eq!(pointer.press(None, 1.0), Press::DragWindow);
        assert_eq!(pointer.press(None, 1.2), Press::Maximize);
    }

    #[test]
    fn native_decorations_leave_the_top_client_row_clickable() {
        let mut pointer = pointer_at(400.0, 10.0).with_titlebar(0.0);
        assert_eq!(pointer.press(None, 1.0), Press::Click);
        assert_eq!(pointer.press(None, 1.2), Press::Click);
        assert_eq!(pointer.press(Some(Target::Surface), 1.3), Press::Click);
    }

    #[test]
    fn title_bar_backgrounds_do_not_block_window_gestures() {
        for target in [
            Target::Surface,
            Target::Settings(SettingsTarget::Surface),
            Target::Routines(RoutinesTarget::Surface),
            Target::Review(ReviewTarget::Surface),
            Target::Memory(MemoryTarget::Surface),
            Target::Update(UpdateTarget::Surface),
            Target::Lock(LockTarget::Surface),
            Target::GetStarted(GetStartedTarget::Surface),
        ] {
            let mut pointer = pointer_at(150.0, 10.0);
            assert_eq!(pointer.press(Some(target), 1.0), Press::DragWindow, "{target:?}");
            assert_eq!(pointer.press(Some(target), 1.2), Press::Maximize, "{target:?}");
            pointer.position.y = titlebar::HEIGHT;
            assert_eq!(pointer.press(Some(target), 2.0), Press::Click, "{target:?}");
        }
    }

    #[test]
    fn double_click_follows_the_system_choice() {
        let mut pointer = pointer_at(400.0, 10.0);
        pointer.double_click = DoubleClickAction::Minimize;
        pointer.press(None, 1.0);
        assert_eq!(pointer.press(None, 1.1), Press::Minimize);
    }

    #[test]
    fn canvas_and_controls_are_plain_clicks() {
        let mut pointer = pointer_at(400.0, 300.0);
        assert_eq!(pointer.press(None, 1.0), Press::Click);
        let mut top = pointer_at(400.0, 10.0);
        assert_eq!(top.press(Some(Target::AddReference), 1.0), Press::Click);
    }

    #[test]
    fn the_effort_meter_is_a_slider() {
        let track = Rect::new(100.0, 0.0, 100.0, 10.0);
        assert_eq!(effort_at(track, 90.0), Effort::Low);
        assert_eq!(effort_at(track, 126.0), Effort::Medium);
        assert_eq!(effort_at(track, 150.0), Effort::High);
        assert_eq!(effort_at(track, 300.0), Effort::Max);
        let mut pointer = pointer_at(400.0, 300.0);
        assert_eq!(pointer.press(Some(Target::Composer(ComposerPart::Effort)), 1.0), Press::Effort);
        assert!(pointer.sliding_effort);
    }

    #[test]
    fn a_second_click_on_a_session_renames_it() {
        let id = superfik_core::ConversationId::new();
        let mut pointer = pointer_at(100.0, 300.0);
        assert_eq!(pointer.press(Some(Target::Conversation(id)), 1.0), Press::Click);
        assert_eq!(pointer.press(Some(Target::Conversation(id)), 1.2), Press::Rename(id));
        assert_eq!(pointer.press(Some(Target::Conversation(id)), 3.0), Press::Click, "too slow");
    }

    #[test]
    fn nodes_are_plain_clicks() {
        let mut pointer = pointer_at(400.0, 300.0);
        assert_eq!(pointer.press(Some(Target::Node(NodeKey::Skills)), 1.0), Press::Click);
    }
}
