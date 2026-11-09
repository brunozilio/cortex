//! Canvas layout: where the composer, the context column, the agent
//! column and the research node go. Pure geometry, unit-tested.

use superfik_render::Rect;
use superfik_ui::theme::metrics;

use super::routing::trunk_ranks;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NodeKey {
    Repository,
    PullRequest,
    Skills,
    Connections,
    Commands,
    References,
    Notes,
    Memory,
    Research,
    Context,
    Agent(usize),
    Browser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Left,
    Right,
    Below,
    Beside,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub key: NodeKey,
    pub column: Column,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasLayout {
    pub composer: Rect,
    pub transcript_x: f32,
    pub transcript_bottom: f32,
    pub slots: Vec<Slot>,
}

fn stack(heights: &[f32], center_y: f32) -> Vec<f32> {
    let total: f32 = heights.iter().sum::<f32>() + metrics::NODE_GAP * heights.len().saturating_sub(1) as f32;
    let mut y = (center_y - total * 0.5).round();
    heights
        .iter()
        .map(|h| {
            let top = y;
            y += h + metrics::NODE_GAP;
            top
        })
        .collect()
}

fn gaps(composer: Rect, tops: &[f32], heights: &[f32]) -> Vec<f32> {
    let mids: Vec<f32> = tops.iter().zip(heights).map(|(t, h)| t + h * 0.5).collect();
    trunk_ranks(composer, &mids)
        .into_iter()
        .map(|rank| match rank {
            None => metrics::COLUMN_FAR,
            Some(0) => metrics::COLUMN_NEAR,
            Some(rank) => metrics::COLUMN_FAR + (rank - 1) as f32 * metrics::TRUNK_STEP,
        })
        .collect()
}

pub fn layout(
    area: Rect,
    left: &[(NodeKey, f32)],
    right: &[(NodeKey, f32)],
    research: Option<f32>,
    beside: Option<f32>,
) -> CanvasLayout {
    let composer = Rect::new(
        (area.x + (area.w - metrics::COMPOSER_WIDTH) * 0.5).round(),
        (area.y + (area.h * 0.5 - 50.0).max(200.0)).round(),
        metrics::COMPOSER_WIDTH,
        metrics::COMPOSER_HEIGHT,
    );
    let center_y = composer.y + metrics::STACK_CENTER;
    let mut slots = Vec::new();

    let heights: Vec<f32> = left.iter().map(|(_, h)| *h).collect();
    let tops = stack(&heights, center_y);
    for ((key, height), (top, gap)) in left.iter().zip(tops.iter().copied().zip(gaps(composer, &tops, &heights))) {
        let rect = Rect::new(composer.x - gap - metrics::NODE_WIDTH, top, metrics::NODE_WIDTH, *height);
        slots.push(Slot { key: *key, column: Column::Left, rect });
    }

    let heights: Vec<f32> = right.iter().map(|(_, h)| *h).collect();
    let tops = stack(&heights, center_y);
    for ((key, height), (top, gap)) in right.iter().zip(tops.iter().copied().zip(gaps(composer, &tops, &heights))) {
        let rect = Rect::new(composer.right() + gap, top, metrics::NODE_WIDTH, *height);
        slots.push(Slot { key: *key, column: Column::Right, rect });
    }

    if let (Some(height), Some(parent)) = (beside, slots.iter().find(|s| s.key == NodeKey::Repository)) {
        let rect = Rect::new(
            parent.rect.x - metrics::BESIDE_GAP - metrics::NODE_WIDTH,
            parent.rect.y,
            metrics::NODE_WIDTH,
            height,
        );
        slots.push(Slot { key: NodeKey::PullRequest, column: Column::Beside, rect });
    }
    if let Some(height) = research {
        let rect = Rect::new(composer.x, composer.bottom() + metrics::RESEARCH_GAP, metrics::NODE_WIDTH, height);
        slots.push(Slot { key: NodeKey::Research, column: Column::Below, rect });
    }

    CanvasLayout { composer, transcript_x: composer.x + 3.0, transcript_bottom: composer.y - 22.5, slots }
}

pub fn place_browser(layout: &mut CanvasLayout, height: f32) {
    let composer = layout.composer;
    let agents = layout.slots.iter().filter(|s| s.column == Column::Right).map(|s| s.rect.right()).reduce(f32::max);
    let x = match agents {
        Some(right) => right + metrics::COLUMN_NEAR,
        None => composer.right() + metrics::COLUMN_FAR,
    };
    let y = (composer.y + metrics::STACK_CENTER - height * 0.5).round();
    let rect = Rect::new(x, y, metrics::BROWSER_WIDTH, height);
    layout.slots.push(Slot { key: NodeKey::Browser, column: Column::Right, rect });
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Rect = Rect::new(0.0, 0.0, 1838.0, 1080.0);

    #[test]
    fn the_browser_sits_right_of_the_composer_or_beyond_the_agents() {
        let mut alone = layout(VIEWPORT, &[], &[], None, None);
        place_browser(&mut alone, 500.0);
        let card = alone.slots[0].rect;
        assert_eq!(card.x - alone.composer.right(), metrics::COLUMN_FAR);
        assert_eq!(card.w, metrics::BROWSER_WIDTH);
        assert!(((card.center().y) - (alone.composer.y + metrics::STACK_CENTER)).abs() <= 0.5);
        let mut beside = layout(VIEWPORT, &[], &[(NodeKey::Agent(0), 100.0)], None, None);
        place_browser(&mut beside, 500.0);
        let agent = beside.slots[0].rect;
        assert_eq!(beside.slots[1].rect.x - agent.right(), metrics::COLUMN_NEAR);
    }

    #[test]
    fn composer_is_centred_like_the_reference() {
        let l = layout(VIEWPORT, &[], &[], None, None);
        assert_eq!(l.composer, Rect::new(699.0, 490.0, 440.0, 190.0));
        let beside_sidebar = layout(Rect::new(272.0, 0.0, 1566.0, 1080.0), &[], &[], None, None);
        assert_eq!(beside_sidebar.composer.x, 835.0, "centred in the area right of the sidebar");
    }

    #[test]
    fn outer_nodes_sit_near_and_inner_nodes_step_out() {
        let left = [
            (NodeKey::Repository, 139.0),
            (NodeKey::Skills, 150.0),
            (NodeKey::References, 85.0),
            (NodeKey::Research, 179.0),
        ];
        let l = layout(VIEWPORT, &left, &[], None, None);
        let gaps: Vec<f32> = l.slots.iter().map(|s| l.composer.x - s.rect.right()).collect();
        let (near, far) = (metrics::COLUMN_NEAR, metrics::COLUMN_FAR);
        assert_eq!(gaps, vec![near, far, far, near]);
        assert!(l.slots.iter().all(|s| s.column == Column::Left));
    }

    #[test]
    fn the_pull_request_hangs_left_of_the_repository_node() {
        let left = [(NodeKey::Repository, 139.0), (NodeKey::Skills, 85.0)];
        let l = layout(VIEWPORT, &left, &[], None, Some(100.0));
        let repo = l.slots.iter().find(|s| s.key == NodeKey::Repository).expect("repository").rect;
        let pr = l.slots.iter().find(|s| s.key == NodeKey::PullRequest).expect("pull request");
        assert_eq!(pr.column, Column::Beside);
        assert_eq!(pr.rect.y, repo.y);
        assert_eq!(repo.x - pr.rect.right(), metrics::BESIDE_GAP);
        assert!(layout(VIEWPORT, &[], &[], None, Some(100.0)).slots.is_empty(), "nothing to hang from");
    }

    #[test]
    fn stack_is_centred_on_the_composer() {
        let left = [(NodeKey::Repository, 100.0), (NodeKey::Skills, 100.0)];
        let l = layout(VIEWPORT, &left, &[], None, None);
        let top = l.slots[0].rect.y;
        let bottom = l.slots[1].rect.bottom();
        assert!(((top + bottom) * 0.5 - (l.composer.y + metrics::STACK_CENTER)).abs() <= 0.5);
        assert_eq!(l.slots[1].rect.y - l.slots[0].rect.bottom(), metrics::NODE_GAP);
    }

    #[test]
    fn research_sits_below_the_composer() {
        let l = layout(VIEWPORT, &[], &[], Some(185.0), None);
        let slot = &l.slots[0];
        assert_eq!(slot.rect, Rect::new(699.0, 710.0, 216.0, 185.0));
        assert_eq!(slot.column, Column::Below);
    }

    #[test]
    fn memory_context_and_agents_stack_right_of_the_composer() {
        let right = [(NodeKey::Memory, 90.0), (NodeKey::Context, 300.0), (NodeKey::Agent(0), 100.0)];
        let l = layout(VIEWPORT, &[], &right, None, None);
        assert!(l.slots.iter().all(|s| s.column == Column::Right && s.rect.x > l.composer.right()));
        let keys: Vec<NodeKey> = l.slots.iter().map(|s| s.key).collect();
        assert_eq!(keys, vec![NodeKey::Memory, NodeKey::Context, NodeKey::Agent(0)]);
        assert!(l.slots[0].rect.bottom() < l.slots[1].rect.y, "stacked in order");
    }
}
