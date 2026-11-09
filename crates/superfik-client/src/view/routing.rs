//! Connector routing: which composer port each node uses and where its
//! connector turns. Pure geometry, recomputed every frame from the nodes'
//! current rects so dragged nodes stay wired.
//!
//! Ports are spread along the composer edge in the nodes' vertical order.
//! Nodes above their port turn on nested trunks — the outermost node on the
//! trunk nearest the composer — so connectors never cross; nodes below are
//! nested the same way from the bottom.

use superfik_render::{Point, Rect, StepPath};
use superfik_ui::theme::metrics;

use super::layout::{Column, NodeKey};

const TUCK: f32 = 6.0;
const BELOW_PORT_X: f32 = 91.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wire {
    pub key: NodeKey,
    pub column: Column,
    pub rect: Rect,
}

pub fn routes(composer: Rect, wires: &[Wire]) -> Vec<(NodeKey, StepPath)> {
    let mut out = Vec::with_capacity(wires.len());
    for column in [Column::Left, Column::Right] {
        let mut side: Vec<&Wire> = wires.iter().filter(|w| w.column == column).collect();
        side.sort_by(|a, b| a.rect.center().y.total_cmp(&b.rect.center().y));
        let ports = ports(composer, side.len());
        let mids: Vec<f32> = side.iter().map(|w| w.rect.center().y).collect();
        for ((wire, port), rank) in side.iter().zip(&ports).zip(ranks(&mids, &ports)) {
            out.push((wire.key, side_path(composer, wire, *port, rank)));
        }
    }
    for wire in wires.iter().filter(|w| w.column == Column::Below) {
        out.push((wire.key, below_path(composer, wire)));
    }
    let parent = wires.iter().find(|w| w.key == NodeKey::Repository).map(|w| w.rect);
    for wire in wires.iter().filter(|w| w.column == Column::Beside) {
        if let Some(parent) = parent {
            out.push((wire.key, beside_path(parent, wire)));
        }
    }
    out
}

fn beside_path(parent: Rect, wire: &Wire) -> StepPath {
    let rect = wire.rect;
    let mid = rect.center().y.round();
    let node = Point::new(rect.right() - TUCK, mid);
    let port_y = mid.clamp(parent.y + metrics::NODE_HEADER * 0.5, parent.bottom() - metrics::NODE_HEADER * 0.5).round();
    let end = Point::new(parent.x + TUCK, port_y);
    if (port_y - mid).abs() < 0.5 {
        return StepPath::new(vec![node, end]);
    }
    StepPath::horizontal(node, end, ((rect.right() + parent.x) * 0.5).round())
}

fn ports(composer: Rect, count: usize) -> Vec<f32> {
    let center = composer.y + metrics::STACK_CENTER;
    (0..count).map(|i| center + (i as f32 - (count as f32 - 1.0) * 0.5) * metrics::PORT_SPACING).collect()
}

pub fn trunk_ranks(composer: Rect, mids: &[f32]) -> Vec<Option<usize>> {
    ranks(mids, &ports(composer, mids.len()))
}

fn ranks(mids: &[f32], ports: &[f32]) -> Vec<Option<usize>> {
    let straight = |i: usize| (mids[i] - ports[i]).abs() < metrics::PORT_SPACING * 0.5;
    let mut ranks = vec![None; mids.len()];
    let above: Vec<usize> = (0..mids.len()).filter(|&i| !straight(i) && mids[i] < ports[i]).collect();
    let below: Vec<usize> = (0..mids.len()).rev().filter(|&i| !straight(i) && mids[i] > ports[i]).collect();
    for group in [above, below] {
        for (rank, i) in group.into_iter().enumerate() {
            ranks[i] = Some(rank);
        }
    }
    ranks
}

fn side_path(composer: Rect, wire: &Wire, port: f32, rank: Option<usize>) -> StepPath {
    let rect = wire.rect;
    let mid = rect.center().y;
    let left = wire.column == Column::Left;
    let (node_x, composer_x) =
        if left { (rect.right() - TUCK, composer.x + TUCK) } else { (rect.x + TUCK, composer.right() - TUCK) };
    let Some(rank) = rank else {
        let y = mid.round();
        return if left {
            StepPath::new(vec![Point::new(node_x, y), Point::new(composer_x, y)])
        } else {
            StepPath::new(vec![Point::new(composer_x, y), Point::new(node_x, y)])
        };
    };
    let inset = metrics::TRUNK_INSET + rank as f32 * metrics::TRUNK_STEP;
    let (edge, gap) = if left { (composer.x, rect.right()) } else { (composer.right(), rect.x) };
    let mut turn = if left { edge - inset } else { edge + inset };
    if (turn - gap).abs() < metrics::CONNECTOR_CORNER * 2.0 || (turn < gap) == left {
        turn = (edge + gap) * 0.5;
    }
    let node = Point::new(node_x, mid);
    let composer_end = Point::new(composer_x, port);
    if left { StepPath::horizontal(node, composer_end, turn) } else { StepPath::horizontal(composer_end, node, turn) }
}

fn below_path(composer: Rect, wire: &Wire) -> StepPath {
    let rect = wire.rect;
    let from = Point::new(composer.x + BELOW_PORT_X, composer.bottom() - TUCK);
    let to = Point::new(rect.x + BELOW_PORT_X, rect.y + TUCK);
    let turn = ((composer.bottom() + rect.y) * 0.5).round();
    StepPath::vertical(from, to, turn)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPOSER: Rect = Rect::new(500.0, 400.0, 440.0, 190.0);

    fn left(key: NodeKey, y: f32, h: f32) -> Wire {
        Wire { key, column: Column::Left, rect: Rect::new(COMPOSER.x - 110.0 - 216.0, y, 216.0, h) }
    }

    fn path(routes: &[(NodeKey, StepPath)], key: NodeKey) -> Vec<Point> {
        routes.iter().find(|(k, _)| *k == key).map(|(_, p)| p.points().to_vec()).unwrap_or_default()
    }

    #[test]
    fn a_beside_node_runs_into_the_repository_not_the_composer() {
        let repo = Wire { key: NodeKey::Repository, column: Column::Left, rect: Rect::new(400.0, 100.0, 216.0, 200.0) };
        let pr = Wire { key: NodeKey::PullRequest, column: Column::Beside, rect: Rect::new(150.0, 100.0, 216.0, 80.0) };
        let r = routes(COMPOSER, &[repo, pr]);
        let points = path(&r, NodeKey::PullRequest);
        assert_eq!(points.len(), 2, "level with the parent: straight");
        assert_eq!(points[0], Point::new(366.0 - 6.0, 140.0));
        assert_eq!(points[1], Point::new(406.0, 140.0));
        let low = Wire { rect: Rect::new(150.0, 400.0, 216.0, 80.0), ..pr };
        assert_eq!(path(&routes(COMPOSER, &[repo, low]), NodeKey::PullRequest).len(), 4, "steps up into the parent");
    }

    #[test]
    fn ports_are_spread_in_node_order() {
        let wires = [left(NodeKey::Skills, 420.0, 100.0), left(NodeKey::Repository, 200.0, 100.0)];
        let r = routes(COMPOSER, &wires);
        let repo = path(&r, NodeKey::Repository);
        let skills = path(&r, NodeKey::Skills);
        let (repo_port, skills_port) = (repo[repo.len() - 1].y, skills[skills.len() - 1].y);
        assert_eq!(skills_port - repo_port, metrics::PORT_SPACING);
        assert_eq!(repo[repo.len() - 1].x, COMPOSER.x + TUCK);
    }

    #[test]
    fn outer_nodes_take_the_inner_trunk_so_wires_never_cross() {
        let wires = [
            left(NodeKey::Repository, 100.0, 100.0),
            left(NodeKey::Skills, 230.0, 100.0),
            left(NodeKey::References, 520.0, 60.0),
            left(NodeKey::Research, 600.0, 160.0),
        ];
        let r = routes(COMPOSER, &wires);
        let trunk = |key| path(&r, key)[1].x;
        assert_eq!(trunk(NodeKey::Repository), COMPOSER.x - metrics::TRUNK_INSET);
        assert_eq!(trunk(NodeKey::Skills), COMPOSER.x - metrics::TRUNK_INSET - metrics::TRUNK_STEP);
        assert_eq!(trunk(NodeKey::Research), COMPOSER.x - metrics::TRUNK_INSET);
        assert_eq!(trunk(NodeKey::References), COMPOSER.x - metrics::TRUNK_INSET - metrics::TRUNK_STEP);
    }

    #[test]
    fn stacked_nodes_rank_from_the_outside_in() {
        let ranks = trunk_ranks(COMPOSER, &[150.0, 300.0, 560.0, 700.0]);
        assert_eq!(ranks, vec![Some(0), Some(1), Some(1), Some(0)]);
    }

    #[test]
    fn a_node_level_with_its_port_runs_straight() {
        let port = ports(COMPOSER, 1)[0];
        let r = routes(COMPOSER, &[left(NodeKey::Skills, port - 50.0, 100.0)]);
        assert_eq!(path(&r, NodeKey::Skills).len(), 2);
    }

    #[test]
    fn agents_are_fed_from_the_composer() {
        let agent =
            Wire { key: NodeKey::Agent(0), column: Column::Right, rect: Rect::new(1000.0, 200.0, 216.0, 110.0) };
        let points = path(&routes(COMPOSER, &[agent]), NodeKey::Agent(0));
        assert_eq!(points[0].x, COMPOSER.right() - TUCK);
        assert_eq!(points[1].x, COMPOSER.right() + metrics::TRUNK_INSET);
        assert_eq!(points[points.len() - 1].x, 1000.0 + TUCK);
    }

    #[test]
    fn a_node_dragged_near_the_composer_turns_halfway() {
        let near = Wire { key: NodeKey::Skills, column: Column::Left, rect: Rect::new(270.0, 100.0, 216.0, 80.0) };
        let points = path(&routes(COMPOSER, &[near]), NodeKey::Skills);
        assert_eq!(points[1].x, (COMPOSER.x + 486.0) * 0.5);
    }

    #[test]
    fn research_hangs_straight_below_until_moved() {
        let rect = Rect::new(COMPOSER.x, COMPOSER.bottom() + 30.0, 216.0, 150.0);
        let wire = Wire { key: NodeKey::Research, column: Column::Below, rect };
        assert_eq!(path(&routes(COMPOSER, &[wire]), NodeKey::Research).len(), 2);
        let moved = Wire { rect: rect.translate(80.0, 40.0), ..wire };
        assert_eq!(path(&routes(COMPOSER, &[moved]), NodeKey::Research).len(), 4);
    }
}
