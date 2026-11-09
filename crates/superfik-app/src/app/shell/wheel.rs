//! The wheel and trackpad scroll: whatever is under the pointer scrolls,
//! pans or zooms, and paged nodes flip (`input::wheel` says where,
//! `input::scrolls` keeps what moves).

use winit::event::{MouseScrollDelta, TouchPhase};

use super::App;
use superfik_client::input::wheel::route_wheel;

const LINE: f32 = 40.0;

impl App {
    pub(super) fn wheel(&mut self, delta: MouseScrollDelta, phase: TouchPhase) {
        let (dx, dy, trackpad) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x * LINE, y * LINE, None),
            MouseScrollDelta::PixelDelta(p) => {
                let scale = self.gpu.as_ref().map_or(1.0, |g| g.window.scale_factor());
                ((p.x / scale) as f32, (p.y / scale) as f32, Some(phase))
            }
        };
        let zooming = self.modifiers.super_key() || self.modifiers.control_key();
        if !self.session.settings.open && !self.session.routines.open && !zooming && self.page_wheel(dx, dy, trackpad) {
            return;
        }
        let at = self.pointer.position;
        let wheel = route_wheel(&self.session, &self.hits, at, self.canvas_pointer(), (dx, dy), zooming, self.hovered);
        let scrolled = self.scrolls.apply(wheel, (dx, dy), at);
        let before = self.session.active;
        for action in scrolled.actions {
            self.dispatch(action);
        }
        if scrolled.stepped_session && self.session.active != before {
            self.haptics.feel(superfik_haptics::Haptic::Level, self.now());
        }
        if let Some(factor) = scrolled.zoom {
            self.camera.zoom_at(factor, at);
        }
        if let Some((dx, dy)) = scrolled.pan {
            self.camera.pan_by(dx, dy);
        }
        self.update_hover();
        self.request_redraw();
    }
}
