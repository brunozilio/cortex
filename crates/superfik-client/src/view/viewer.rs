//! The reference viewer: the reference open in `expanded_reference`,
//! shown over the whole window. Painted in window coordinates (the second
//! render pass, after the sidebar) so it sits above everything, and its
//! scrim registers as a screen hit that swallows the canvas underneath.

use std::collections::HashMap;
use std::path::PathBuf;

use superfik_render::Rect;
use superfik_ui::components::ImageViewer;
use superfik_ui::{Painter, Plane};

use super::LoadedImage;
use super::motions::Motions;
use super::nodes::references;
use super::target::{HitMap, Target};
use crate::state::Session;

pub struct ViewerView<'a> {
    pub session: &'a Session,
    pub images: &'a HashMap<PathBuf, LoadedImage>,
    pub full: &'a HashMap<PathBuf, LoadedImage>,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub now: f64,
}

impl ViewerView<'_> {
    pub fn paint(&self, p: &mut Painter, motions: &mut Motions, tile: Option<Rect>, hits: &mut HitMap) {
        let c = self.session.conversation();
        let open = c.expanded_reference.filter(|i| *i < c.references.len());
        let viewport = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        let centre = viewport.center();
        let from = tile.or_else(|| motions.viewer(self.now).is_none().then(|| Rect::new(centre.x, centre.y, 0.0, 0.0)));
        motions.sync_viewer(open, from, self.now);
        let Some(viewer) = motions.viewer(self.now) else { return };
        let Some(path) = c.references.get(viewer.index) else { return };
        let view = ImageViewer {
            tile: self.tile(path),
            from: viewer.from,
            viewport,
            progress: viewer.progress(self.now),
            close_hovered: self.hovered == Some(Target::CloseViewer),
        };
        let painted = p.on_plane(Plane::Overlay, |p| view.paint(p));
        if viewer.presence.is_visible() {
            hits.add_screen(painted.scrim, Target::Scrim);
            hits.add_screen(painted.frame, Target::Surface);
            hits.add_screen(painted.close, Target::CloseViewer);
        }
    }
}

impl<'a> ViewerView<'a> {
    fn tile(&self, path: &'a std::path::Path) -> superfik_ui::components::Tile<'a> {
        references::tile(path, if self.full.contains_key(path) { self.full } else { self.images })
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
        let view = ViewerView {
            session,
            images: &HashMap::new(),
            full: &HashMap::new(),
            hovered: None,
            viewport: (1200.0, 800.0),
            now,
        };
        view.paint(&mut p, motions, Some(Rect::new(40.0, 300.0, 60.0, 60.0)), &mut hits);
        hits
    }

    #[test]
    fn a_reference_with_no_tile_on_screen_opens_from_the_centre() {
        let mut session = Session::new(None);
        session.apply(Action::AddReferences(vec!["/a/notes.md".into()]), 0.0);
        session.apply(Action::ToggleReference(0), 0.0);
        let mut motions = Motions::default();
        let mut scene = Scene::new(superfik_render::Color::TRANSPARENT, LAYERS);
        let mut text = superfik_render::TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &superfik_ui::theme::colors::DARK);
        let view = ViewerView {
            session: &session,
            images: &HashMap::new(),
            full: &HashMap::new(),
            hovered: None,
            viewport: (1200.0, 800.0),
            now: 0.0,
        };
        view.paint(&mut p, &mut motions, None, &mut HitMap::default());
        assert_eq!(motions.viewer(0.0).expect("viewer").from, Rect::new(600.0, 400.0, 0.0, 0.0));
    }

    #[test]
    fn open_viewer_covers_the_window_and_closing_lets_clicks_through() {
        let mut session = Session::new(None);
        session.apply(Action::AddReferences(vec!["/a/notes.md".into()]), 0.0);
        let mut motions = Motions::default();
        let at = |hits: &HitMap, x, y| hits.at(Point::new(x, y), Point::new(x, y));
        assert_eq!(at(&paint(&session, &mut motions, 0.0), 10.0, 10.0), None);
        session.apply(Action::ToggleReference(0), 0.0);
        let hits = paint(&session, &mut motions, 1.0);
        assert_eq!(at(&hits, 10.0, 10.0), Some(Target::Scrim));
        let hits = paint(&session, &mut motions, 2.0);
        assert_eq!(at(&hits, 600.0, 400.0), Some(Target::Surface), "the open file swallows clicks");
        assert_eq!(at(&hits, 1200.0 - 22.0, 22.0), Some(Target::CloseViewer));
        session.apply(Action::ToggleReference(0), 2.0);
        let hits = paint(&session, &mut motions, 2.05);
        assert!(motions.viewer(2.05).is_some(), "still shrinking back");
        assert_eq!(at(&hits, 10.0, 10.0), None);
    }

    #[test]
    fn the_viewer_shows_the_full_size_copy_once_loaded() {
        use superfik_render::ImageId;
        use superfik_ui::components::Tile;
        let session = Session::new(None);
        let path = PathBuf::from("/a/sky.png");
        let thumbs = HashMap::from([(path.clone(), LoadedImage { id: ImageId(1), aspect: 1.5 })]);
        let full = HashMap::from([(path.clone(), LoadedImage { id: ImageId(2), aspect: 1.5 })]);
        let none = HashMap::new();
        let view = |full| ViewerView {
            session: &session,
            images: &thumbs,
            full,
            hovered: None,
            viewport: (1.0, 1.0),
            now: 0.0,
        };
        assert!(matches!(view(&none).tile(&path), Tile::Image { id: ImageId(1), .. }), "thumbnail until then");
        assert!(matches!(view(&full).tile(&path), Tile::Image { id: ImageId(2), .. }));
    }
}
