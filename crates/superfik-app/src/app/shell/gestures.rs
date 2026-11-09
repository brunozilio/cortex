//! Input handling for the shell: pointer presses, the effort slide, hover and keys.

use superfik_haptics::Haptic;
use superfik_render::{Point, Rect};
use winit::window::CursorIcon;

use super::App;
use crate::app::input::KeyCommand;
use crate::app::pointer::Press;
use superfik_client::input::press::{Pressed, dragged, pressed};
use superfik_client::input::route::{Route, route};

use crate::state::Action;
use crate::view::layout::NodeKey;
use crate::view::spacing::Attached;
use crate::view::target::Target;

impl App {
    fn viewport(&self) -> (f32, f32) {
        self.gpu.as_ref().map_or((0.0, 0.0), |gpu| {
            let size = gpu.window.inner_size().to_logical::<f32>(gpu.window.scale_factor());
            (size.width, size.height)
        })
    }

    fn window_center(&self) -> Point {
        let (w, h) = self.viewport();
        Point::new(w * 0.5, h * 0.5)
    }

    pub(super) fn memory_graph_under_pointer(&self) -> Option<Rect> {
        self.hits.memory_graph.filter(|graph| self.session.memory.open && graph.contains(self.pointer.position))
    }

    pub(super) fn canvas_pointer(&self) -> Point {
        self.camera.to_canvas(self.pointer.position)
    }

    pub(super) fn update_hover(&mut self) {
        let hovered = self.hits.at(self.pointer.position, self.canvas_pointer());
        self.show_cursor();
        if hovered != self.hovered {
            self.hovered = hovered;
            self.show_cursor();
            self.request_redraw();
        }
    }

    fn cursor(&self) -> CursorIcon {
        if let Some(cursor) = self.page_cursor() {
            return cursor;
        }
        match self.hovered {
            _ if self.pointer.sliding_effort => CursorIcon::EwResize,
            Some(Target::Input) => CursorIcon::Text,
            Some(Target::Composer(superfik_ui::components::ComposerPart::Effort)) => CursorIcon::EwResize,
            Some(target) if target.is_button() => CursorIcon::Pointer,
            _ => CursorIcon::Default,
        }
    }

    pub(super) fn show_cursor(&mut self) {
        let cursor = self.cursor();
        if self.shown_cursor != Some(cursor)
            && let Some(gpu) = &self.gpu
        {
            gpu.window.set_cursor(cursor);
            self.shown_cursor = Some(cursor);
        }
    }

    pub(super) fn key(&mut self, command: KeyCommand) {
        match route(&self.session, &self.hits, command) {
            Route::Act(action) => self.dispatch(action),
            Route::Paste(into) => {
                if let Ok(text) = self.clipboard.get_text() {
                    self.dispatch(into(text));
                }
            }
            Route::Copy { text, then } => {
                let _ = self.clipboard.set_text(text);
                if let Some(action) = then {
                    self.dispatch(action);
                }
            }
            Route::Zoom(factor) => {
                self.camera.zoom_at(factor, self.window_center());
                self.request_redraw();
            }
            Route::ResetView => {
                self.camera.reset();
                self.request_redraw();
            }
            Route::Nothing => {}
        }
    }

    pub(super) fn press(&mut self) {
        self.tooltip.dismiss();
        let target = self.hits.at(self.pointer.position, self.canvas_pointer());
        let window = self.gpu.as_ref().map(|g| g.window.clone());
        let press = self.pointer.press(target, self.now());
        let at = self.canvas_pointer();
        let press = match pressed(&self.session, &self.hits, press, target, at, self.modifiers.shift_key()) {
            Pressed::Act(actions) => return actions.into_iter().for_each(|action| self.dispatch(action)),
            Pressed::Recentre => {
                let area = crate::view::sidebar::canvas_area(self.viewport(), self.sidebar.width);
                self.camera.centre_on(self.base.composer, area);
                return self.request_redraw();
            }
            Pressed::Window(press) => press,
        };
        match press {
            Press::DragWindow => {
                if let Some(Err(error)) = window.map(|w| w.drag_window()) {
                    tracing::debug!(%error, "window drag unavailable");
                }
            }
            Press::Maximize => {
                if let Some(window) = window {
                    window.set_maximized(!window.is_maximized());
                }
            }
            Press::Minimize => {
                if let Some(window) = window {
                    window.set_minimized(true);
                }
            }
            Press::Effort | Press::Rename(_) | Press::Text { .. } | Press::Click => {}
        }
    }

    pub(super) fn pointer_moved(&mut self) {
        let at = self.canvas_pointer();
        for action in dragged(&self.session, &self.hits, &self.pointer, at) {
            if matches!(action, Action::SetEffort(_)) {
                self.haptics.feel(Haptic::Level, self.now());
            }
            self.dispatch(action);
        }
    }
}

pub(super) fn collision_sets(attached: &[(Option<NodeKey>, Rect)], composer: Rect) -> (Vec<Attached>, Vec<Rect>) {
    let extras = attached.iter().filter_map(|(key, rect)| key.map(|k| (k, *rect))).collect();
    let avoid = std::iter::once(composer)
        .chain(attached.iter().filter(|(key, _)| key.is_none()).map(|(_, rect)| rect.translate(composer.x, composer.y)))
        .collect();
    (extras, avoid)
}
