use superfik_render::{Color, Scene, TextSystem};

use super::*;
use crate::painter::LAYERS;
use crate::theme::colors;

#[test]
fn the_backdrop_runs_the_window_height_and_the_rows_stop_above_the_footer() {
    let mut scene = Scene::new(Color::TRANSPARENT, LAYERS);
    let mut text = TextSystem::new();
    let mut p = Painter::new(&mut scene, &mut text, 1.0, &colors::DARK);
    let window = Rect::new(0.0, 0.0, 1200.0, 800.0);
    let timeline = Timeline {
        top_inset: metrics::TITLEBAR_HEIGHT,
        groups: &[],
        hovered: None,
        marker: None,
        scroll: 0.0,
        time: 0.0,
        new_label: "New",
        footer: 48.0,
        shortcuts: None,
    };
    let hits = timeline.paint(&mut p, window);
    assert_eq!((hits.painted.y, hits.painted.h), (0.0, 800.0));
    assert!(hits.rows.iter().all(|(rect, _)| rect.bottom() <= 800.0 - 48.0));
}

#[test]
fn holding_command_shows_hints_without_changing_the_rows() {
    let mut scene = Scene::new(Color::TRANSPARENT, LAYERS);
    let mut text = TextSystem::new();
    let mut p = Painter::new(&mut scene, &mut text, 1.0, &colors::DARK);
    let window = Rect::new(0.0, 0.0, 1200.0, 800.0);
    let item = TimelineItemProps {
        title: "Fix the build",
        running: false,
        waiting: false,
        selected: false,
        archived: false,
        editing: None,
        shortcut: Some(2),
        attention: None,
    };
    let items = [item];
    let groups = [TimelineGroupProps { label: "Today", collapsed: false, items: &items }];
    let paint = |p: &mut Painter, shortcuts: Option<&str>| {
        Timeline {
            top_inset: metrics::TITLEBAR_HEIGHT,
            groups: &groups,
            hovered: None,
            marker: None,
            scroll: 0.0,
            time: 0.0,
            new_label: "New",
            footer: 48.0,
            shortcuts,
        }
        .paint(p, window)
    };
    let plain = paint(&mut p, None);
    let held = paint(&mut p, Some("⌘"));
    assert_eq!(plain.rows, held.rows, "the same rows are hit whether or not ⌘ is held");
    assert_eq!(plain.rows.len(), 3, "New, the day and the session");
}
