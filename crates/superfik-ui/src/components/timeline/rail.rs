//! Geometry of the timeline rail: a vertical line on the outer rail at
//! each day group that bends to the inner rail for its sessions and back.
//! Pure, so the bends and the travelling dot's path are testable.

use std::f32::consts::{FRAC_PI_2, PI};

use superfik_render::{GooShape, Point};

use crate::theme::metrics;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Piece {
    Vertical { x: f32, top: f32, bottom: f32 },
    Arc { center: Point, radius: f32, start: f32, sweep: f32 },
}

impl Piece {
    fn ends(&self) -> (f32, f32) {
        match self {
            Self::Vertical { top, bottom, .. } => (*top, *bottom),
            Self::Arc { center, radius, start, sweep } => {
                (center.y + radius * start.sin(), center.y + radius * (start + sweep).sin())
            }
        }
    }

    pub fn top(&self) -> f32 {
        let (a, b) = self.ends();
        a.min(b)
    }

    pub fn bottom(&self) -> f32 {
        let (a, b) = self.ends();
        a.max(b)
    }

    fn x_at(&self, y: f32) -> f32 {
        match self {
            Self::Vertical { x, .. } => *x,
            Self::Arc { center, radius, start, .. } => {
                let dy = ((y - center.y) / radius).clamp(-1.0, 1.0);
                let dx = (1.0 - dy * dy).sqrt() * radius;
                let mid = start + FRAC_PI_2 * 0.5;
                if mid.cos() >= 0.0 { center.x + dx } else { center.x - dx }
            }
        }
    }

    fn clamped(&self, top: f32, bottom: f32) -> Option<Self> {
        match *self {
            Self::Vertical { x, top: t, bottom: b } => {
                let (t, b) = (t.max(top), b.min(bottom));
                (b > t).then_some(Self::Vertical { x, top: t, bottom: b })
            }
            arc => (arc.bottom() > top && arc.top() < bottom).then_some(arc),
        }
    }

    pub fn shape(&self, half_width: f32) -> GooShape {
        match *self {
            Self::Vertical { x, top, bottom } => {
                GooShape::Capsule { a: Point::new(x, top), b: Point::new(x, bottom), radius: half_width }
            }
            Self::Arc { center, radius, start, sweep } => {
                GooShape::Arc { center, radius, start, sweep, thickness: half_width }
            }
        }
    }
}

pub fn group_rail(group_y: f32, item_ys: &[f32], next_group_y: Option<f32>) -> Vec<Piece> {
    let (outer, inner) = (metrics::TIMELINE_GROUP_X, metrics::TIMELINE_ITEM_X);
    let r = (inner - outer) * 0.5;
    let mut pieces = Vec::new();
    let Some((&first, &last)) = item_ys.first().zip(item_ys.last()) else {
        if let Some(next) = next_group_y {
            pieces.push(Piece::Vertical { x: outer, top: group_y, bottom: next });
        }
        return pieces;
    };
    let bend = group_y + metrics::TIMELINE_BEND_GAP;
    pieces.push(Piece::Vertical { x: outer, top: group_y, bottom: bend });
    pieces.push(Piece::Arc { center: Point::new(outer + r, bend), radius: r, start: FRAC_PI_2, sweep: FRAC_PI_2 });
    pieces.push(Piece::Arc {
        center: Point::new(inner - r, bend + 2.0 * r),
        radius: r,
        start: PI + FRAC_PI_2,
        sweep: FRAC_PI_2,
    });
    pieces.push(Piece::Vertical { x: inner, top: (bend + 2.0 * r).min(first), bottom: last });
    if let Some(next) = next_group_y {
        let bend = last + metrics::TIMELINE_BEND_GAP;
        pieces.push(Piece::Vertical { x: inner, top: last, bottom: bend });
        pieces.push(Piece::Arc { center: Point::new(inner - r, bend), radius: r, start: 0.0, sweep: FRAC_PI_2 });
        pieces.push(Piece::Arc {
            center: Point::new(outer + r, bend + 2.0 * r),
            radius: r,
            start: PI,
            sweep: FRAC_PI_2,
        });
        pieces.push(Piece::Vertical { x: outer, top: (bend + 2.0 * r).min(next), bottom: next });
    }
    pieces
}

pub fn x_at(pieces: &[Piece], y: f32) -> f32 {
    pieces
        .iter()
        .find(|p| y >= p.top() - 0.01 && y <= p.bottom() + 0.01)
        .map_or(metrics::TIMELINE_ITEM_X, |p| p.x_at(y))
}

pub fn between(pieces: &[Piece], top: f32, bottom: f32) -> Vec<Piece> {
    let (top, bottom) = (top.min(bottom), top.max(bottom));
    pieces.iter().filter_map(|p| p.clamped(top, bottom)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTER: f32 = metrics::TIMELINE_GROUP_X;
    const INNER: f32 = metrics::TIMELINE_ITEM_X;

    #[test]
    fn a_group_bends_in_to_its_sessions_and_back_out() {
        let pieces = group_rail(100.0, &[138.0, 176.0], Some(214.0));
        assert_eq!(pieces.len(), 8);
        assert_eq!(pieces[0], Piece::Vertical { x: OUTER, top: 100.0, bottom: 108.0 });
        assert!(matches!(pieces[3], Piece::Vertical { x: INNER, top, bottom: 176.0 } if top == 124.0));
        assert!(matches!(pieces[7], Piece::Vertical { x: OUTER, bottom: 214.0, .. }));
        for w in pieces.windows(2) {
            assert!((w[0].bottom() - w[1].top()).abs() < 0.01, "{w:?}");
        }
    }

    #[test]
    fn the_last_group_ends_at_its_last_session() {
        let pieces = group_rail(100.0, &[138.0], None);
        assert_eq!(pieces.len(), 4);
        assert_eq!(pieces.last().map(Piece::bottom), Some(138.0));
        assert!(group_rail(100.0, &[], None).is_empty());
        assert_eq!(group_rail(100.0, &[], Some(140.0)), vec![Piece::Vertical { x: OUTER, top: 100.0, bottom: 140.0 }]);
    }

    #[test]
    fn x_follows_the_bend_from_outer_to_inner() {
        let pieces = group_rail(100.0, &[138.0], None);
        assert_eq!(x_at(&pieces, 104.0), OUTER);
        assert_eq!(x_at(&pieces, 130.0), INNER);
        let mid = x_at(&pieces, 116.0);
        assert!((mid - (OUTER + INNER) * 0.5).abs() < 0.01, "{mid}");
        assert!(x_at(&pieces, 110.0) < mid && mid < x_at(&pieces, 122.0));
    }

    #[test]
    fn between_cuts_verticals_and_keeps_overlapping_arcs() {
        let pieces = group_rail(100.0, &[138.0, 176.0], None);
        let trail = between(&pieces, 176.0, 150.0);
        assert_eq!(trail, vec![Piece::Vertical { x: INNER, top: 150.0, bottom: 176.0 }]);
        let across = between(&pieces, 104.0, 130.0);
        assert_eq!(across.len(), 4);
        assert_eq!(across[0], Piece::Vertical { x: OUTER, top: 104.0, bottom: 108.0 });
    }
}
