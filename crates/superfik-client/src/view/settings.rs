//! The buttons at the foot of the sidebar: the gear that opens the Settings
//! screen and the plug that opens the Plugins screen. Painted in window
//! coordinates, like the sidebar.

use superfik_render::{Point, Quad, Rect};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, Plane, icons};

use super::target::{HitMap, Target};
use crate::state::Session;

pub fn plugins_rect(viewport: (f32, f32)) -> Rect {
    let gear = gear_rect(viewport);
    Rect::new(gear.right() + metrics::SETTINGS_BUTTON_GAP, gear.y, gear.w, gear.h)
}

pub fn routines_rect(viewport: (f32, f32)) -> Rect {
    let plug = plugins_rect(viewport);
    Rect::new(plug.right() + metrics::SETTINGS_BUTTON_GAP, plug.y, plug.w, plug.h)
}

pub fn update_rect(viewport: (f32, f32)) -> Rect {
    let routines = routines_rect(viewport);
    Rect::new(routines.right() + metrics::SETTINGS_BUTTON_GAP, routines.y, routines.w, routines.h)
}

pub fn gear_rect(viewport: (f32, f32)) -> Rect {
    let size = metrics::SETTINGS_BUTTON;
    let center = Point::new(metrics::TIMELINE_GROUP_X, viewport.1 - metrics::SETTINGS_FOOTER * 0.5);
    Rect::new(center.x - size * 0.5, center.y - size * 0.5, size, size)
}

pub struct SettingsView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
}

impl SettingsView<'_> {
    pub fn paint(&self, p: &mut Painter, viewport: (f32, f32), hits: &mut HitMap) {
        let gear = gear_rect(viewport);
        let lit = self.session.settings.open || self.hovered == Some(Target::OpenSettings);
        button(p, gear, icons::GEAR, lit);
        hits.add_screen(gear, Target::OpenSettings);
        let plug = plugins_rect(viewport);
        let lit = self.session.plugins_pane_open() || self.hovered == Some(Target::OpenPlugins);
        button(p, plug, icons::PLUG, lit);
        hits.add_screen(plug, Target::OpenPlugins);
        let routines = routines_rect(viewport);
        let lit = self.session.routines.open || self.hovered == Some(Target::OpenRoutines);
        button(p, routines, icons::LOOP, lit);
        if self.session.unread_reports() > 0 {
            corner_dot(p, routines, p.palette().attention);
        }
        hits.add_screen(routines, Target::OpenRoutines);
        if self.session.update.offered() {
            let update = update_rect(viewport);
            let target = Target::Update(super::target::UpdateTarget::Open);
            button(p, update, icons::ARROW_UP, self.hovered == Some(target));
            if self.session.update.ready() {
                corner_dot(p, update, p.palette().form_accent);
            }
            hits.add_screen(update, target);
        }
    }
}

fn corner_dot(p: &mut Painter, rect: Rect, color: superfik_render::Color) {
    p.on_plane(Plane::Overlay, |p| {
        let r = metrics::FOOTER_DOT_RADIUS;
        let center = Point::new(rect.right() - metrics::FOOTER_DOT_INSET, rect.y + metrics::FOOTER_DOT_INSET);
        p.quad(Quad::new(Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0), r, color));
    });
}

fn button(p: &mut Painter, rect: Rect, icon: superfik_render::Icon, lit: bool) {
    p.on_plane(Plane::Overlay, |p| {
        if lit {
            p.quad(Quad::new(rect, metrics::ROW_RADIUS, p.palette().timeline_hover));
        }
        let tint = if lit { p.palette().icon } else { p.palette().icon_muted };
        p.icon_at(icon, rect.center(), metrics::ICON_LARGE, Some(tint));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gear_sits_in_the_footer_on_the_timeline_axis() {
        let gear = gear_rect((1200.0, 800.0));
        assert_eq!(gear.center().x, metrics::TIMELINE_GROUP_X);
        assert!(gear.bottom() <= 800.0 && gear.y >= 800.0 - metrics::SETTINGS_FOOTER);
    }
}
