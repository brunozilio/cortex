//! Keeping canvas nodes apart. A node that appears on top of another node,
//! the composer or the sidebar's column is given the nearest clear place
//! and keeps it; a panel open off a node moves with it and counts as part
//! of it. `panel_edge` and `right_edge` tell the camera how far a pan may
//! go before content disappears under the sidebar.

use std::collections::{HashMap, HashSet};

use superfik_render::{Point, Rect};
use superfik_ui::components::{EarSide, NodeFrame};
use superfik_ui::theme::metrics;

use super::layout::{NodeKey, Slot};

const TOUCH: f32 = 0.5;

pub type Attached = (NodeKey, Rect);

fn ear_side(key: NodeKey) -> Option<EarSide> {
    match key {
        NodeKey::Repository | NodeKey::PullRequest | NodeKey::References | NodeKey::Skills => Some(EarSide::Left),
        NodeKey::Agent(_) | NodeKey::Browser => Some(EarSide::Right),
        NodeKey::Research
        | NodeKey::Context
        | NodeKey::Memory
        | NodeKey::Notes
        | NodeKey::Connections
        | NodeKey::Commands => None,
    }
}

fn footprint(key: NodeKey, rect: Rect, extras: &[Attached]) -> Vec<Rect> {
    let ear = ear_side(key).filter(|_| rect.h >= metrics::NODE_HEADER).map(|side| NodeFrame::ear_rect(rect, side));
    std::iter::once(rect)
        .chain(ear)
        .chain(extras.iter().filter(|(k, _)| *k == key).map(|(_, r)| r.translate(rect.x, rect.y)))
        .collect()
}

#[derive(Debug, Default)]
pub struct NodeSpacing {
    offsets: HashMap<NodeKey, Point>,
    seen: HashSet<NodeKey>,
}

impl NodeSpacing {
    pub fn place_new(&mut self, slots: &[Slot], extras: &[Attached], walls: &[Rect], avoid: &[Rect]) -> Vec<NodeKey> {
        let mut moved = Vec::new();
        for slot in slots {
            if !self.seen.insert(slot.key) {
                continue;
            }
            let home = self.placed(slot);
            let blockers: Vec<Rect> = slots
                .iter()
                .filter(|s| s.key != slot.key)
                .flat_map(|s| footprint(s.key, self.placed(s), extras))
                .chain(avoid.iter().copied())
                .chain(walls.iter().copied())
                .collect();
            let mut parts = footprint(slot.key, home, extras);
            let mut origin = Point::new(home.x, home.y);
            for _ in 0..4 {
                let next = unstick(&parts, &blockers, &[]);
                if next == origin {
                    break;
                }
                for rect in &mut parts {
                    *rect = rect.translate(next.x - origin.x, next.y - origin.y);
                }
                origin = next;
            }
            if origin != Point::new(home.x, home.y) {
                let inherited = self.inherited_offset(slot.key);
                self.offsets.insert(
                    slot.key,
                    Point::new(origin.x - slot.rect.x - inherited.x, origin.y - slot.rect.y - inherited.y),
                );
                moved.push(slot.key);
            }
        }
        moved
    }

    pub fn panel_edge(&self, slots: &[Slot], extras: &[Attached]) -> Option<f32> {
        slots
            .iter()
            .flat_map(|s| {
                let at = self.placed(s);
                extras.iter().filter(move |(k, _)| *k == s.key).map(move |(_, r)| r.translate(at.x, at.y))
            })
            .map(|r| r.x - metrics::NODE_CLEARANCE)
            .reduce(f32::min)
    }

    pub fn right_edge(&self, slots: &[Slot], extras: &[Attached]) -> Option<f32> {
        slots
            .iter()
            .flat_map(|s| footprint(s.key, self.placed(s), extras))
            .map(|r| r.right() + metrics::NODE_CLEARANCE)
            .reduce(f32::max)
    }

    fn inherited_offset(&self, key: NodeKey) -> Point {
        if key == NodeKey::PullRequest {
            self.offsets.get(&NodeKey::Repository).copied().unwrap_or_default()
        } else {
            Point::default()
        }
    }

    fn placed(&self, slot: &Slot) -> Rect {
        let offset = self.offsets.get(&slot.key).copied().unwrap_or_default();
        let inherited = self.inherited_offset(slot.key);
        slot.rect.translate(offset.x + inherited.x, offset.y + inherited.y)
    }

    pub fn apply(&self, slots: &mut [Slot]) {
        for slot in slots {
            slot.rect = self.placed(slot);
        }
    }

    pub fn reset(&mut self) {
        self.offsets.clear();
        self.seen.clear();
    }

    #[cfg(test)]
    pub fn take_offsets(&mut self) -> HashMap<NodeKey, Point> {
        let offsets = std::mem::take(&mut self.offsets);
        self.reset();
        offsets
    }

    #[cfg(test)]
    pub fn restore_offsets(&mut self, offsets: HashMap<NodeKey, Point>) {
        self.reset();
        self.offsets = offsets;
    }
}

fn unstick(parts: &[Rect], walls: &[Rect], others: &[Rect]) -> Point {
    let gap = metrics::NODE_CLEARANCE;
    let mut parts = parts.to_vec();
    for wall in walls {
        let Some(ways) = escapes(&parts, wall.inset(-gap)) else { continue };
        let blocked = |p: &Point| {
            let moved: Vec<Rect> = parts.iter().map(|r| r.translate(p.x, p.y)).collect();
            others.iter().chain(walls).any(|o| overlaps(&moved, o.inset(-gap)))
        };
        let push = ways.iter().find(|p| !blocked(p)).copied().unwrap_or(ways[0]);
        for rect in &mut parts {
            *rect = rect.translate(push.x, push.y);
        }
    }
    parts.first().map_or(Point::default(), |r| Point::new(r.x, r.y))
}

fn overlaps(parts: &[Rect], other: Rect) -> bool {
    parts.iter().any(|rect| {
        rect.x + TOUCH < other.right()
            && other.x + TOUCH < rect.right()
            && rect.y + TOUCH < other.bottom()
            && other.y + TOUCH < rect.bottom()
    })
}

fn escapes(parts: &[Rect], wall: Rect) -> Option<[Point; 4]> {
    let inside: Vec<&Rect> = parts.iter().filter(|r| overlaps(std::slice::from_ref(r), wall)).collect();
    if inside.is_empty() {
        return None;
    }
    let most = |f: &dyn Fn(&Rect) -> f32| inside.iter().map(|r| f(r)).fold(0.0, f32::max).ceil();
    let mut ways = [
        Point::new(most(&|r| wall.right() - r.x), 0.0),
        Point::new(-most(&|r| r.right() - wall.x), 0.0),
        Point::new(0.0, most(&|r| wall.bottom() - r.y)),
        Point::new(0.0, -most(&|r| r.bottom() - wall.y)),
    ];
    ways.sort_by(|a, b| (a.x.abs() + a.y.abs()).total_cmp(&(b.x.abs() + b.y.abs())));
    Some(ways)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::layout::Column;

    const GAP: f32 = metrics::NODE_CLEARANCE;
    const EAR: f32 = 29.0;
    const OPEN: &[Rect] = &[];
    const NONE: &[Attached] = &[];

    fn slot(key: NodeKey, x: f32, y: f32) -> Slot {
        Slot { key, column: Column::Left, rect: Rect::new(x, y, 200.0, 100.0) }
    }

    fn at(spacing: &NodeSpacing, slots: &[Slot]) -> Vec<Rect> {
        let mut moved = slots.to_vec();
        spacing.apply(&mut moved);
        moved.into_iter().map(|s| s.rect).collect()
    }

    #[test]
    fn offsets_survive_relayout_and_reset() {
        let mut spacing = NodeSpacing::default();
        spacing.restore_offsets(HashMap::from([(NodeKey::Skills, Point::new(30.0, -20.0))]));
        let slots = [Slot { key: NodeKey::Skills, column: Column::Left, rect: Rect::new(100.0, 140.0, 216.0, 60.0) }];
        assert_eq!(at(&spacing, &slots)[0], Rect::new(130.0, 120.0, 216.0, 60.0));
        assert_eq!(spacing.take_offsets().get(&NodeKey::Skills), Some(&Point::new(30.0, -20.0)));
        assert_eq!(at(&spacing, &slots)[0], slots[0].rect);
    }

    #[test]
    fn a_pull_request_opens_beside_the_settled_repository() {
        let repo = slot(NodeKey::Repository, 500.0, 300.0);
        let pr = Slot {
            key: NodeKey::PullRequest,
            column: Column::Beside,
            rect: Rect::new(repo.rect.x - metrics::BESIDE_GAP - 200.0, repo.rect.y, 200.0, 80.0),
        };
        let mut spacing = NodeSpacing::default();
        spacing.restore_offsets(HashMap::from([(NodeKey::Repository, Point::new(400.0, 60.0))]));
        spacing.place_new(&[repo], NONE, OPEN, &[]);
        spacing.place_new(&[repo, pr], NONE, OPEN, &[]);
        let moved = at(&spacing, &[repo, pr]);
        assert!((metrics::BESIDE_GAP..=metrics::BESIDE_GAP + 1.0).contains(&(moved[0].x - moved[1].right())));
        assert_eq!(moved[0].y, moved[1].y);
    }

    #[test]
    fn a_pull_request_keeps_its_own_clearance_after_inheriting_the_repository_offset() {
        let repo = slot(NodeKey::Repository, 500.0, 300.0);
        let pr = Slot { key: NodeKey::PullRequest, column: Column::Beside, rect: Rect::new(262.0, 300.0, 200.0, 80.0) };
        let mut spacing = NodeSpacing::default();
        spacing.restore_offsets(HashMap::from([(NodeKey::Repository, Point::new(400.0, 60.0))]));
        spacing.place_new(&[repo], NONE, OPEN, &[]);
        let obstacle = Rect::new(660.0, 360.0, 200.0, 80.0);
        assert_eq!(spacing.place_new(&[repo, pr], NONE, OPEN, &[obstacle]), vec![NodeKey::PullRequest]);
        let moved = at(&spacing, &[repo, pr]);
        assert!((metrics::BESIDE_GAP..=metrics::BESIDE_GAP + 1.0).contains(&(moved[0].x - moved[1].right())));
        assert!(!overlaps(&[moved[1]], obstacle.inset(-GAP + 1.0)));
        assert!(spacing.place_new(&[repo, pr], NONE, OPEN, &[obstacle]).is_empty());
        assert_eq!(at(&spacing, &[repo, pr]), moved);
    }

    #[test]
    fn a_panel_open_off_a_node_counts_as_part_of_it() {
        let picker = (NodeKey::Skills, Rect::new(-254.0, 20.0, 240.0, 150.0));
        let slots = [slot(NodeKey::Skills, 300.0, 50.0)];
        let spacing = NodeSpacing::default();
        assert_eq!(spacing.right_edge(&slots, &[picker]), Some(300.0 + 200.0 + GAP));
        assert_eq!(spacing.right_edge(&[], NONE), None);
        assert_eq!(spacing.panel_edge(&slots, &[picker]), Some(300.0 - 254.0 - GAP));
        assert_eq!(spacing.panel_edge(&slots, NONE), None, "the nodes alone make no room");
        let slots = [slot(NodeKey::Skills, 600.0, 50.0), slot(NodeKey::References, 400.0, 150.0)];
        let mut spacing = NodeSpacing::default();
        spacing.place_new(&slots[..1], &[picker], OPEN, &[]);
        assert_eq!(spacing.place_new(&slots, &[picker], OPEN, &[]), vec![NodeKey::References]);
        let moved = at(&spacing, &slots);
        assert!(!overlaps(&[moved[1]], Rect::new(346.0, 70.0, 240.0, 150.0).inset(-GAP + 1.0)));
    }

    #[test]
    fn a_node_that_appears_in_the_sidebar_column_is_placed_beside_it() {
        let column = Rect::new(-1000.0, -5000.0, 1160.0, 10000.0);
        let slots = [slot(NodeKey::Skills, 130.0, 300.0)];
        let mut spacing = NodeSpacing::default();
        assert_eq!(spacing.place_new(&slots, NONE, &[column], &[]), vec![NodeKey::Skills]);
        assert_eq!(at(&spacing, &slots)[0].x, 160.0 + EAR + GAP);
    }

    #[test]
    fn a_node_that_appears_on_another_is_given_a_clear_place_and_keeps_it() {
        let composer = Rect::new(600.0, 0.0, 440.0, 190.0);
        let mut spacing = NodeSpacing::default();
        let first = [slot(NodeKey::Skills, 100.0, 100.0)];
        assert!(spacing.place_new(&first, NONE, OPEN, &[composer]).is_empty(), "a clean layout moves nothing");
        spacing.restore_offsets(HashMap::from([(NodeKey::Skills, Point::new(250.0, 150.0))]));
        spacing.place_new(&first, NONE, OPEN, &[composer]);
        let slots = [slot(NodeKey::Skills, 100.0, 100.0), slot(NodeKey::Agent(0), 380.0, 220.0)];
        assert_eq!(spacing.place_new(&slots, NONE, OPEN, &[composer]), vec![NodeKey::Agent(0)]);
        let moved = at(&spacing, &slots);
        assert_eq!(moved[0], Rect::new(350.0, 250.0, 200.0, 100.0));
        assert_eq!(moved[1], Rect::new(380.0, 350.0 + GAP, 200.0, 100.0));
        assert!(spacing.place_new(&slots, NONE, OPEN, &[composer]).is_empty(), "seen nodes are left alone");
        spacing.reset();
        assert!(spacing.place_new(&first, NONE, OPEN, &[composer]).is_empty());
    }

    #[test]
    fn a_new_node_leaving_one_blocker_does_not_stop_on_another() {
        let composer = Rect::new(500.0, 0.0, 440.0, 400.0);
        let slots = [
            slot(NodeKey::Skills, 100.0, 100.0),
            slot(NodeKey::References, 100.0, 170.0),
            slot(NodeKey::Agent(0), 150.0, 150.0),
        ];
        let mut spacing = NodeSpacing::default();
        spacing.restore_offsets(HashMap::from([
            (NodeKey::Skills, Point::default()),
            (NodeKey::References, Point::default()),
        ]));
        spacing.place_new(&slots[..2], NONE, OPEN, &[composer]);
        assert_eq!(spacing.place_new(&slots, NONE, OPEN, &[composer]), vec![NodeKey::Agent(0)]);
        let moved = at(&spacing, &slots);
        for other in [moved[0], moved[1], composer] {
            assert!(!overlaps(&[moved[2]], other.inset(-GAP + 1.0)), "agent at {:?} on {:?}", moved[2], other);
        }
    }
}
