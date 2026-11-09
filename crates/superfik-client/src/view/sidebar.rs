//! The sidebar: the session timeline, fixed to the window's left edge. It
//! is painted in window coordinates (a separate render pass, so the camera
//! never moves or zooms it) and its rows register as screen hits.

use superfik_i18n::Text;
use superfik_render::Rect;
use superfik_ui::components::{AttentionMark, Timeline, TimelineGroupProps, TimelineHit, TimelineItemProps};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, Plane};

use super::motions::Motions;
use super::settings::SettingsView;
use super::target::{HitMap, Target};
use crate::state::{Attention, Session, TimelineGroup};

pub fn canvas_area(viewport: (f32, f32), width: f32) -> Rect {
    let width = width.clamp(metrics::SIDEBAR_MIN_WIDTH, metrics::SIDEBAR_MAX_WIDTH);
    Rect::new(width, 0.0, (viewport.0 - width).max(1.0), viewport.1)
}

pub fn off_screen(rect: Rect, area: Rect) -> bool {
    rect.right() <= area.x || rect.x >= area.right() || rect.bottom() <= area.y || rect.y >= area.bottom()
}

pub fn beside(rect: Rect, edge: f32) -> f32 {
    if rect.w <= 0.0 {
        return if rect.x >= edge { 1.0 } else { 0.0 };
    }
    ((rect.right() - edge) / rect.w).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SidebarExtent {
    pub width: f32,
    pub content_height: f32,
    pub selected_y: Option<f32>,
}

pub fn reveal(scroll: f32, y: f32, height: f32, overflow: f32) -> f32 {
    let margin = metrics::TIMELINE_ROW;
    let shift = if y < margin {
        y - margin
    } else if y > height - margin {
        y - (height - margin)
    } else {
        0.0
    };
    (scroll + shift).clamp(0.0, overflow.max(0.0))
}

pub struct SidebarView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub scroll: f32,
    pub caret_visible: bool,
    pub now: f64,
    pub shortcuts: bool,
}

impl SidebarView<'_> {
    pub fn paint(
        &self,
        p: &mut Painter,
        viewport: (f32, f32),
        motions: &mut Motions,
        hits: &mut HitMap,
    ) -> SidebarExtent {
        let area = Rect::new(0.0, 0.0, viewport.0, viewport.1);
        let groups: Vec<TimelineGroup> = self.session.timeline(self.now);
        let renaming = self.session.renaming.as_ref();
        let digits = self.session.shortcut_order(self.now);
        let items: Vec<Vec<TimelineItemProps>> = groups
            .iter()
            .map(|g| {
                g.items
                    .iter()
                    .map(|i| TimelineItemProps {
                        title: &i.title,
                        running: i.running,
                        waiting: i.waiting,
                        selected: i.selected,
                        archived: g.archived,
                        editing: renaming.filter(|(id, _)| *id == i.id).map(|(_, edit)| (edit, self.caret_visible)),
                        shortcut: digits.iter().position(|id| *id == i.id).map(|at| at + 1),
                        attention: i.attention.map(|attention| match attention {
                            Attention::Asking => AttentionMark::Asking,
                            Attention::Failed => AttentionMark::Failed,
                            Attention::Done => AttentionMark::Done,
                        }),
                    })
                    .collect()
            })
            .collect();
        let props: Vec<TimelineGroupProps> = groups
            .iter()
            .zip(&items)
            .map(|(g, items)| TimelineGroupProps { label: &g.label, collapsed: g.collapsed, items })
            .collect();
        let locate = |id| {
            groups
                .iter()
                .enumerate()
                .find_map(|(group, g)| g.items.iter().position(|i| i.id == id).map(|item| (group, item)))
        };
        let hovered = match self.hovered {
            Some(Target::NewConversation) => Some(TimelineHit::New),
            Some(Target::DayGroup(day)) => groups.iter().position(|g| g.day == day).map(TimelineHit::Group),
            Some(Target::ArchivedGroup) => groups.iter().position(|g| g.archived).map(TimelineHit::Group),
            Some(Target::Conversation(id) | Target::ArchivedConversation(id)) => {
                locate(id).map(|(group, item)| TimelineHit::Item { group, item })
            }
            Some(Target::ArchiveConversation(id) | Target::UnarchiveConversation(id)) => {
                locate(id).map(|(group, item)| TimelineHit::Archive { group, item })
            }
            _ => None,
        };
        let new_label = self.session.strings().t(Text::NewSession);
        let timeline = Timeline {
            top_inset: self.session.platform.titlebar_height(),
            groups: &props,
            hovered,
            marker: motions.timeline_marker(self.now),
            scroll: self.scroll,
            time: self.now as f32,
            new_label: &new_label,
            footer: metrics::SETTINGS_FOOTER,
            shortcuts: self.shortcuts.then(|| self.session.platform.primary.label()),
        };
        let rows = p.on_plane(Plane::Overlay, |p| timeline.paint(p, area));
        let selected = rows.item_ys.iter().find_map(|(hit, y)| match hit {
            TimelineHit::Item { group, item } if groups[*group].items[*item].selected => Some(*y),
            _ => None,
        });
        motions.sync_timeline(selected, self.now);
        let painted = rows.painted;
        hits.add_screen(painted, Target::Surface);
        for (rect, hit) in rows.rows {
            let target = match hit {
                TimelineHit::New => Target::NewConversation,
                TimelineHit::Group(group) if groups[group].archived => Target::ArchivedGroup,
                TimelineHit::Group(group) => Target::DayGroup(groups[group].day),
                TimelineHit::Item { group, item } if groups[group].archived => {
                    Target::ArchivedConversation(groups[group].items[item].id)
                }
                TimelineHit::Item { group, item } => Target::Conversation(groups[group].items[item].id),
                TimelineHit::Archive { group, item } if groups[group].archived => {
                    Target::UnarchiveConversation(groups[group].items[item].id)
                }
                TimelineHit::Archive { group, item } => Target::ArchiveConversation(groups[group].items[item].id),
            };
            hits.add_screen(rect.intersect(&painted), target);
        }
        SettingsView { session: self.session, hovered: self.hovered }.paint(p, viewport, hits);
        SidebarExtent { width: painted.w, content_height: rows.content_height, selected_y: selected }
    }
}

#[cfg(test)]
mod off_screen_tests {
    use super::*;

    #[test]
    fn a_rect_wholly_outside_the_canvas_is_off_screen_and_any_overlap_is_not() {
        let area = Rect::new(200.0, 0.0, 800.0, 600.0);
        assert!(off_screen(Rect::new(0.0, 100.0, 150.0, 80.0), area), "left of the sidebar edge");
        assert!(off_screen(Rect::new(1100.0, 100.0, 150.0, 80.0), area), "past the right");
        assert!(off_screen(Rect::new(400.0, 700.0, 150.0, 80.0), area), "below the window");
        assert!(!off_screen(Rect::new(150.0, 100.0, 150.0, 80.0), area), "a sliver still in view");
        assert!(!off_screen(Rect::new(400.0, 100.0, 150.0, 80.0), area));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROW: f32 = metrics::TIMELINE_ROW;

    #[test]
    fn a_row_in_view_keeps_the_scroll() {
        assert_eq!(reveal(40.0, 200.0, 400.0, 500.0), 40.0);
    }

    #[test]
    fn a_row_below_scrolls_up_into_view() {
        assert_eq!(reveal(0.0, 450.0, 400.0, 500.0), 450.0 - (400.0 - ROW));
    }

    #[test]
    fn a_row_above_scrolls_down_into_view_but_never_past_the_top() {
        assert_eq!(reveal(100.0, 0.0, 400.0, 500.0), 100.0 - ROW);
        assert_eq!(reveal(10.0, 0.0, 400.0, 500.0), 0.0);
    }

    #[test]
    fn reveal_never_scrolls_past_the_end() {
        assert_eq!(reveal(0.0, 900.0, 400.0, 120.0), 120.0);
    }

    #[test]
    fn a_node_fades_the_further_it_slides_under_the_sidebar() {
        let node = Rect::new(100.0, 0.0, 200.0, 80.0);
        assert_eq!(beside(node, 100.0), 1.0, "clear of it");
        assert_eq!(beside(node, 150.0), 0.75, "a quarter under");
        assert_eq!(beside(node, 200.0), 0.5, "half under");
        assert_eq!(beside(node, 300.0), 0.0, "all the way under");
        assert_eq!(beside(node, 900.0), 0.0);
        assert_eq!(beside(Rect::new(100.0, 0.0, 0.0, 0.0), 150.0), 0.0, "no width");
    }
}
