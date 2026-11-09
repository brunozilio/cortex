//! What changed between two frames of the same size.

use crate::Pixels;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn area(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }
}

pub fn damage(previous: &[u8], current: &Pixels<'_>, tile: u32) -> Vec<Rect> {
    let (width, height) = (current.width, current.height);
    let row_bytes = width as usize * 4;
    let mut runs: Vec<Rect> = Vec::new();
    let mut ty = 0;
    while ty < height {
        let th = tile.min(height - ty);
        let mut row_runs: Vec<Rect> = Vec::new();
        let mut tx = 0;
        while tx < width {
            let tw = tile.min(width - tx);
            let span = tx as usize * 4..(tx + tw) as usize * 4;
            let changed = (ty..ty + th).any(|y| {
                let before = &previous[y as usize * row_bytes..][span.clone()];
                before != &current.row(y)[span.clone()]
            });
            if changed {
                match row_runs.last_mut() {
                    Some(run) if run.x + run.w == tx => run.w += tw,
                    _ => row_runs.push(Rect { x: tx, y: ty, w: tw, h: th }),
                }
            }
            tx += tw;
        }
        for run in row_runs {
            match runs.iter_mut().find(|r| r.x == run.x && r.w == run.w && r.y + r.h == run.y) {
                Some(above) => above.h += run.h,
                None => runs.push(run),
            }
        }
        ty += th;
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Order;

    fn pixels(data: &[u8], width: u32, height: u32) -> Pixels<'_> {
        Pixels { data, width, height, stride: width as usize * 4, order: Order::Rgba }
    }

    #[test]
    fn unchanged_frames_have_no_damage() {
        let frame = vec![7u8; 100 * 50 * 4];
        assert!(damage(&frame, &pixels(&frame, 100, 50), 16).is_empty());
    }

    #[test]
    fn a_changed_pixel_damages_its_tile_and_neighbours_join() {
        let before = vec![0u8; 100 * 50 * 4];
        let mut after = before.clone();
        let set = |data: &mut Vec<u8>, x: usize, y: usize| data[(y * 100 + x) * 4] = 255;
        set(&mut after, 5, 5);
        assert_eq!(damage(&before, &pixels(&after, 100, 50), 16), [Rect { x: 0, y: 0, w: 16, h: 16 }]);
        set(&mut after, 20, 5);
        set(&mut after, 99, 49);
        assert_eq!(
            damage(&before, &pixels(&after, 100, 50), 16),
            [Rect { x: 0, y: 0, w: 32, h: 16 }, Rect { x: 96, y: 48, w: 4, h: 2 }]
        );
    }

    #[test]
    fn runs_over_the_same_columns_join_downwards() {
        let before = vec![0u8; 64 * 64 * 4];
        let mut after = before.clone();
        for y in [1usize, 20, 40] {
            after[(y * 64 + 3) * 4 + 1] = 9;
        }
        assert_eq!(damage(&before, &pixels(&after, 64, 64), 16), [Rect { x: 0, y: 0, w: 16, h: 48 }]);
    }

    #[test]
    fn padding_after_each_row_is_ignored() {
        let before = vec![0u8; 8 * 2 * 4];
        let mut padded = vec![0u8; 64 * 2];
        padded[40] = 1;
        let frame = Pixels { data: &padded, width: 8, height: 2, stride: 64, order: Order::Rgba };
        assert!(damage(&before, &frame, 4).is_empty());
    }
}
