//! The session notice (engine missing, not logged in, run errors), pinned
//! above the canvas over the composer's column, or over a full-window
//! screen's column while one covers the canvas. Clicking × dismisses it.

use superfik_render::{Point, Rect};
use superfik_ui::components::{Notice, NoticeHits};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, Plane};

use super::target::{HitMap, Target};
use crate::state::Session;

pub fn paint(p: &mut Painter, session: &Session, composer: Rect, hovered: Option<Target>, hits: &mut HitMap) {
    let origin = Point::new(composer.x, metrics::NOTICE_TOP);
    if let Some(notice) = paint_at(p, session, origin, composer.w, hovered) {
        hits.add(notice.rect, Target::Surface);
        hits.add(notice.close, Target::DismissNotice);
    }
}

pub fn paint_on_screen(p: &mut Painter, session: &Session, column: Rect, hovered: Option<Target>, hits: &mut HitMap) {
    let origin = Point::new(column.x, column.y + metrics::NOTICE_TOP);
    if let Some(notice) = paint_at(p, session, origin, column.w, hovered) {
        hits.add_screen(notice.rect, Target::Surface);
        hits.add_screen(notice.close, Target::DismissNotice);
    }
}

fn paint_at(
    p: &mut Painter,
    session: &Session,
    origin: Point,
    width: f32,
    hovered: Option<Target>,
) -> Option<NoticeHits> {
    let shown = session.notice.as_ref()?;
    let text = shown.text(&session.strings());
    let notice = Notice {
        text: &text,
        width,
        reveal: 1.0,
        close_hovered: hovered == Some(Target::DismissNotice),
        informative: shown.is_informative(),
    };
    Some(p.on_plane(Plane::Overlay, |p| notice.paint(p, origin)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use superfik_core::PluginId;
    use superfik_protocol::ServerEvent;
    use superfik_render::Scene;
    use superfik_ui::painter::LAYERS;

    use super::*;
    use crate::state::{Action, SettingsPane};
    use crate::view::settings_screen::SettingsScreenView;

    #[test]
    fn a_sign_in_that_fails_in_settings_is_said_there_and_dismissed() {
        let mut session = Session::default();
        session.apply(Action::ShowSettingsPane(SettingsPane::Ai), 0.0);
        session.apply(Action::OpenSettings, 0.0);
        let failed = ServerEvent::PluginFailed {
            plugin: Some(PluginId("openai-subscription".into())),
            message: "ChatGPT access token has no account id".into(),
        };
        session.apply(Action::Server(failed), 0.0);
        let mut scene = Scene::new(superfik_render::Color::TRANSPARENT, LAYERS);
        let mut text = superfik_render::TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &superfik_ui::theme::colors::DARK);
        let mut hits = HitMap::default();
        let icons = HashMap::new();
        let view = SettingsScreenView {
            session: &session,
            hovered: None,
            viewport: (1200.0, 800.0),
            browser_icons: &icons,
            now: 0.0,
            caret_visible: true,
        };
        view.paint(&mut p, &mut hits);
        let close = (0..1200)
            .step_by(4)
            .flat_map(|x| (0..800).step_by(4).map(move |y| Point::new(x as f32, y as f32)))
            .find(|&at| hits.at(at, at) == Some(Target::DismissNotice));
        assert!(close.is_some(), "the notice's close knob is over the Settings screen");
    }
}
