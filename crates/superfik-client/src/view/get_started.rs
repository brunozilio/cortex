//! Get Started over the whole window (first launch): the night intro
//! ("Welcome aboard" and its cast), then the app's face, the title, what
//! Superfik does (each feature in its node's accent) and Continue. Painted
//! in window coordinates, last in the overlay pass; while it shows, every
//! pixel is a screen hit, so what is underneath is inert. It fades away on
//! Continue and lets clicks through as it does.

use superfik_i18n::{Strings, Text};
use superfik_render::Rect;
use superfik_ui::components::{Emote, Feature, GetStarted};
use superfik_ui::emote::EmoteState;
use superfik_ui::theme::colors::Palette;
use superfik_ui::theme::motion;
use superfik_ui::{Painter, Plane, icons};

use super::motions::Motions;
use super::target::{GetStartedTarget, HitMap, Target};
use crate::state::Session;

pub struct GetStartedView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub now: f64,
}

fn copy(strings: &Strings) -> [(String, String); 4] {
    [
        (Text::GetStartedCanvas, Text::GetStartedCanvasBody),
        (Text::GetStartedAgents, Text::GetStartedAgentsBody),
        (Text::GetStartedMemory, Text::GetStartedMemoryBody),
        (Text::GetStartedReview, Text::GetStartedReviewBody),
    ]
    .map(|(title, body)| (strings.t(title), strings.t(body)))
}

fn looks(c: &Palette) -> [(superfik_render::Icon, superfik_render::Color); 4] {
    [(icons::LAYERS, c.skill), (icons::ROBOT, c.agent), (icons::BRAIN, c.memory), (icons::PULL_REQUEST, c.branch)]
}

impl GetStartedView<'_> {
    pub fn paint(&self, p: &mut Painter, motions: &mut Motions, hits: &mut HitMap) -> Option<f32> {
        motions.sync_get_started(self.session.shows_get_started(), self.now);
        let presence = motions.get_started(self.now)?;
        let strings = self.session.strings();
        let copy = copy(&strings);
        let features: Vec<Feature> = copy
            .iter()
            .zip(looks(p.palette()))
            .map(|((title, body), (icon, tint))| Feature { icon, tint, title, body })
            .collect();
        let welcome = strings.t(Text::WelcomeAboard);
        let title = strings.t(Text::GetStarted);
        let action = strings.t(Text::Continue);
        let view = GetStarted {
            welcome: &welcome,
            emote: Emote { state: EmoteState::Idle, from: None, time: self.now as f32, width: 0.0 },
            title: &title,
            features: &features,
            action: &action,
            action_hovered: self.hovered == Some(Target::GetStarted(GetStartedTarget::Continue)),
            elapsed: if presence.is_visible() { presence.elapsed(self.now) } else { motion::GET_STARTED_OPEN },
        };
        let window = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        let opacity = if presence.is_visible() { 1.0 } else { motion::ENTER.apply(presence.progress(self.now)) };
        let painted = p.on_plane(Plane::Canvas, |p| p.faded(opacity, |p| view.paint(p, window)));
        if presence.is_visible() {
            hits.add_screen(window, Target::GetStarted(GetStartedTarget::Surface));
            if let Some(action) = painted.action {
                hits.add_screen(action, Target::GetStarted(GetStartedTarget::Continue));
            }
            return Some(presence.elapsed(self.now));
        }
        None
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
        GetStartedView { session, hovered: None, viewport: (1200.0, 800.0), now }.paint(&mut p, motions, &mut hits);
        hits
    }

    fn at(hits: &HitMap, x: f32, y: f32) -> Option<Target> {
        hits.at(Point::new(x, y), Point::new(x, y))
    }

    #[test]
    fn it_covers_the_window_while_shown_and_lets_clicks_through_as_it_fades() {
        let mut session = Session::default();
        let mut motions = Motions::default();
        assert_eq!(at(&paint(&session, &mut motions, 0.0), 10.0, 10.0), None, "not shown");
        session.apply(Action::ShowGetStarted, 0.0);
        let hits = paint(&session, &mut motions, 0.0);
        assert_eq!(at(&hits, 10.0, 10.0), Some(Target::GetStarted(GetStartedTarget::Surface)));
        let continue_at = |hits: &HitMap| {
            (400..800)
                .map(|y| y as f32)
                .find(|y| at(hits, 600.0, *y) == Some(Target::GetStarted(GetStartedTarget::Continue)))
        };
        assert_eq!(continue_at(&paint(&session, &mut motions, 2.0)), None, "not during the intro");
        let open = f64::from(motion::GET_STARTED_OPEN) + 0.1;
        let hits = paint(&session, &mut motions, open);
        assert!(!motions.is_animating(open), "revealed");
        assert!(continue_at(&hits).is_some(), "Continue sits centred in the lower half");
        session.apply(Action::DismissGetStarted, open);
        let hits = paint(&session, &mut motions, open + 0.05);
        assert!(motions.get_started(open + 0.05).is_some(), "still fading");
        assert_eq!(at(&hits, 10.0, 10.0), None);
        paint(&session, &mut motions, open + 1.0);
        assert!(motions.get_started(open + 1.0).is_none(), "gone");
    }

    #[test]
    fn every_feature_has_its_words_in_both_languages() {
        for language in [crate::state::Language::English, crate::state::Language::Portuguese] {
            let strings = Strings::new(language);
            for (title, body) in copy(&strings) {
                assert!(!title.is_empty() && body.len() > title.len(), "{title}: {body}");
            }
        }
    }
}
