//! A grid of dots, one per slice of a capacity: coloured runs for what is
//! used, faint dots for what is free and rings for what is held back.

use superfik_render::{Color, Quad, Rect};

use crate::painter::Painter;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Used(usize),
    Free,
    Reserved,
}

pub fn cells(segments: &[u64], capacity: u64, reserved: u64, count: usize) -> Vec<Cell> {
    if capacity == 0 || count == 0 {
        return vec![Cell::Free; count];
    }
    let dots = |tokens: u64| {
        let exact = tokens as f64 / capacity as f64 * count as f64;
        if tokens == 0 { 0 } else { (exact.round() as usize).max(1) }
    };
    let held = dots(reserved).min(count);
    let mut runs: Vec<usize> = segments.iter().map(|&tokens| dots(tokens)).collect();
    while runs.iter().sum::<usize>() > count - held {
        let Some(largest) = runs.iter_mut().max() else { break };
        *largest -= 1;
    }
    let used = runs.iter().sum::<usize>();
    let mut out: Vec<Cell> =
        runs.iter().enumerate().flat_map(|(i, &n)| std::iter::repeat_n(Cell::Used(i), n)).collect();
    out.extend(std::iter::repeat_n(Cell::Free, count - held - used));
    out.extend(std::iter::repeat_n(Cell::Reserved, held));
    out
}

#[derive(Debug, Clone, Copy)]
pub struct UsageGrid<'a> {
    pub segments: &'a [(u64, Color)],
    pub capacity: u64,
    pub reserved: u64,
    pub free: Color,
    pub reserved_ring: Color,
}

impl UsageGrid<'_> {
    pub fn height(width: f32) -> f32 {
        let dot = Self::dot(width);
        let rows = metrics::USAGE_ROWS as f32;
        rows * dot + (rows - 1.0) * metrics::USAGE_DOT_GAP
    }

    fn dot(width: f32) -> f32 {
        let columns = metrics::USAGE_COLUMNS as f32;
        (width - (columns - 1.0) * metrics::USAGE_DOT_GAP) / columns
    }

    pub fn paint(&self, p: &mut Painter, rect: Rect) {
        let tokens: Vec<u64> = self.segments.iter().map(|(tokens, _)| *tokens).collect();
        let count = metrics::USAGE_COLUMNS * metrics::USAGE_ROWS;
        let dot = Self::dot(rect.w);
        let step = dot + metrics::USAGE_DOT_GAP;
        for (i, cell) in cells(&tokens, self.capacity, self.reserved, count).into_iter().enumerate() {
            let (column, row) = (i % metrics::USAGE_COLUMNS, i / metrics::USAGE_COLUMNS);
            let at = Rect::new(rect.x + column as f32 * step, rect.y + row as f32 * step, dot, dot);
            let quad = match cell {
                Cell::Used(segment) => Quad::new(at, dot * 0.5, self.segments[segment].1),
                Cell::Free => Quad::new(at, dot * 0.5, self.free),
                Cell::Reserved => Quad::new(at, dot * 0.5, self.free).border(self.reserved_ring, 1.2),
            };
            p.quad(quad);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(cells: &[Cell], cell: Cell) -> usize {
        cells.iter().filter(|c| **c == cell).count()
    }

    #[test]
    fn segments_fill_in_order_and_reserve_sits_at_the_end() {
        let grid = cells(&[4_200, 20_300, 435, 131_500], 1_000_000, 33_000, 100);
        assert_eq!(grid.len(), 100);
        assert_eq!(&grid[..3], &[Cell::Used(0), Cell::Used(1), Cell::Used(1)]);
        assert_eq!(count(&grid, Cell::Used(2)), 1, "tiny segments keep one dot");
        assert_eq!(count(&grid, Cell::Used(3)), 13);
        assert_eq!(count(&grid, Cell::Reserved), 3);
        assert_eq!(&grid[97..], &[Cell::Reserved; 3]);
        assert_eq!(count(&grid, Cell::Free), 100 - 1 - 2 - 1 - 13 - 3);
    }

    #[test]
    fn overflow_takes_dots_from_the_largest_run() {
        let grid = cells(&[150_000, 60_000], 200_000, 20_000, 10);
        assert_eq!(count(&grid, Cell::Reserved), 1);
        assert_eq!((count(&grid, Cell::Used(0)), count(&grid, Cell::Used(1))), (6, 3));
        assert_eq!(count(&grid, Cell::Free), 0);
    }

    #[test]
    fn nothing_measured_is_all_free() {
        assert_eq!(cells(&[], 0, 0, 4), vec![Cell::Free; 4]);
    }
}
