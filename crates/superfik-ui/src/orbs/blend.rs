//! Morphing one frame into another: dots are paired by their angle round
//! the centre and glide to their new place, so a face dissolves into an
//! orb (or an alert) rather than cutting.

use super::core::{Dot, Line, OrbFrame, lerp};

fn by_angle(frame: &OrbFrame, size: f64) -> Vec<Dot> {
    let c = size / 2.0;
    let mut dots = frame.dots.clone();
    dots.sort_by(|a, b| (a.y - c).atan2(a.x - c).total_cmp(&(b.y - c).atan2(b.x - c)));
    dots
}

pub fn blend(from: &OrbFrame, to: &OrbFrame, size: f64, k: f64) -> OrbFrame {
    let k = k.clamp(0.0, 1.0);
    let (a, b) = (by_angle(from, size), by_angle(to, size));
    let n = a.len().max(b.len());
    let mut dots = Vec::with_capacity(n);
    if n == 0 {
        return OrbFrame::default();
    }
    let pick = |v: &[Dot], i: usize| v[(i * v.len()) / n];
    for i in 0..n {
        let (p, q) = match (a.is_empty(), b.is_empty()) {
            (true, _) => (pick(&b, i), pick(&b, i)),
            (_, true) => (pick(&a, i), pick(&a, i)),
            _ => (pick(&a, i), pick(&b, i)),
        };
        dots.push(Dot {
            x: lerp(p.x, q.x, k),
            y: lerp(p.y, q.y, k),
            z: lerp(p.z, q.z, k),
            r: lerp(p.r, q.r, k),
            white: lerp(p.white, q.white, k),
            a: lerp(p.a, q.a, k),
        });
    }
    dots.sort_by(|a, b| a.z.total_cmp(&b.z));
    let fade = |l: &Line, a: f64| Line { a: l.a * a, ..*l };
    let lines = from.lines.iter().map(|l| fade(l, 1.0 - k)).chain(to.lines.iter().map(|l| fade(l, k))).collect();
    OrbFrame { dots, lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(x: f64, y: f64) -> Dot {
        Dot { x, y, z: 0.0, r: 1.0, white: 0.1, a: 1.0 }
    }

    #[test]
    fn ends_match_the_sources_and_the_middle_is_between() {
        let from = OrbFrame { dots: vec![dot(2.0, 10.0), dot(18.0, 10.0)], lines: vec![] };
        let to =
            OrbFrame { dots: vec![dot(10.0, 2.0), dot(10.0, 18.0), dot(2.0, 2.0), dot(18.0, 18.0)], lines: vec![] };
        assert_eq!(blend(&from, &to, 20.0, 0.0).dots.len(), 4);
        let end = blend(&from, &to, 20.0, 1.0);
        assert!(end.dots.iter().all(|d| to.dots.contains(d)));
        let mid = blend(&from, &to, 20.0, 0.5);
        assert!(mid.dots.iter().all(|d| d.x > 1.0 && d.x < 19.0));
    }

    #[test]
    fn empty_sides_are_tolerated() {
        let to = OrbFrame { dots: vec![dot(1.0, 1.0)], lines: vec![] };
        assert_eq!(blend(&OrbFrame::default(), &to, 20.0, 0.3).dots.len(), 1);
        assert!(blend(&OrbFrame::default(), &OrbFrame::default(), 20.0, 0.3).dots.is_empty());
    }
}
