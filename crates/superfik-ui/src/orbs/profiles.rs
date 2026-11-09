//! Density profiles and the multiplier machinery that scales them. The
//! base rows are the library's `fine` profiles; each preset (state × size)
//! applies count and radius multipliers on top.

use std::collections::BTreeMap;

use super::presets::Mode;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opts(BTreeMap<&'static str, f64>);

impl Opts {
    pub fn get(&self, key: &str, default: f64) -> f64 {
        self.0.get(key).copied().unwrap_or(default)
    }

    pub fn try_get(&self, key: &str) -> Option<f64> {
        self.0.get(key).copied()
    }

    pub fn set(&mut self, key: &'static str, value: f64) {
        self.0.insert(key, value);
    }

    pub fn from_pairs(pairs: &[(&'static str, f64)]) -> Self {
        Self(pairs.iter().copied().collect())
    }
}

const COUNT_PAIRS: [(&str, &str); 3] = [("latRings", "lonDensity"), ("rings", "lonDensity"), ("lanes", "segs")];
const COUNT_KEYS: [&str; 5] = ["orbitN", "ghostN", "nodeN", "strandN", "signals"];
const ICON_DENSITY_KEYS: [&str; 1] = ["iconD"];
const RADIUS_KEYS: [&str; 9] =
    ["rBase", "rDepth", "rActive", "rDot", "ghostR", "partR", "partRDepth", "nodeR", "nodeRDepth"];

pub fn scale_counts(opts: &Opts, scale: f64) -> Opts {
    let mut out = opts.clone();
    let mut done: Vec<&str> = Vec::new();
    let rt = scale.sqrt();
    for (a, b) in COUNT_PAIRS {
        if let (Some(va), Some(vb)) = (out.try_get(a), out.try_get(b))
            && !done.contains(&a)
            && !done.contains(&b)
        {
            out.set(a, (va * rt).round().max(2.0));
            out.set(b, (vb * rt).round().max(2.0));
            done.push(a);
            done.push(b);
        }
    }
    for k in COUNT_KEYS {
        if let Some(v) = out.try_get(k)
            && v != 0.0
            && !done.contains(&k)
        {
            out.set(k, (v * scale).round().max(1.0));
        }
    }
    for k in ICON_DENSITY_KEYS {
        if let Some(v) = out.try_get(k) {
            out.set(k, (v * scale).max(0.02));
        }
    }
    out
}

pub fn scale_radii(opts: &Opts, scale: f64) -> Opts {
    let mut out = opts.clone();
    for k in RADIUS_KEYS {
        if let Some(v) = out.try_get(k) {
            out.set(k, v * scale);
        }
    }
    let mul = out.get("rSizeMul", 1.0) * scale;
    out.set("rSizeMul", mul);
    out
}

pub fn base_profile(mode: Mode) -> Opts {
    let pairs: &[(&'static str, f64)] = match mode {
        Mode::Globe => &[
            ("latRings", 17.0),
            ("lonDensity", 44.0),
            ("rBase", 0.6),
            ("rDepth", 1.7),
            ("rBoost", 1.0),
            ("inkFar", 0.62),
            ("inkSpan", 0.54),
            ("rsPow", 0.6),
            ("rMin", 0.3),
        ],
        Mode::Orbits => &[
            ("orbitN", 12.0),
            ("ghostN", 40.0),
            ("ghostR", 0.9),
            ("ghostA", 0.5),
            ("particles", 3.0),
            ("partR", 1.2),
            ("partRDepth", 1.6),
            ("rsPow", 0.6),
            ("rMin", 0.3),
        ],
        Mode::Rubik => &[
            ("latRings", 15.0),
            ("lonDensity", 40.0),
            ("moveCount", 14.0),
            ("rBase", 0.6),
            ("rDepth", 1.7),
            ("rActive", 0.3),
            ("inkFar", 0.62),
            ("inkSpan", 0.54),
            ("rsPow", 0.6),
            ("rMin", 0.3),
        ],
        Mode::Wave => {
            &[("rings", 15.0), ("lonDensity", 40.0), ("rBase", 0.6), ("rDepth", 1.7), ("rsPow", 0.6), ("rMin", 0.3)]
        }
        Mode::Web => &[
            ("nodeN", 30.0),
            ("thr", 0.72),
            ("signals", 5.0),
            ("nodeR", 1.4),
            ("nodeRDepth", 1.8),
            ("lineW", 0.8),
            ("rsPow", 0.6),
            ("rMin", 0.3),
        ],
        Mode::Braid => &[
            ("strandN", 52.0),
            ("turns", 3.0),
            ("ghostN", 150.0),
            ("rBase", 1.2),
            ("rDepth", 1.8),
            ("rsPow", 0.6),
            ("rMin", 0.3),
        ],
        Mode::Ribbon => &[
            ("lanes", 5.0),
            ("segs", 88.0),
            ("ghostN", 150.0),
            ("rBase", 1.1),
            ("rDepth", 1.7),
            ("rsPow", 0.6),
            ("rMin", 0.3),
        ],
        Mode::Ring => &[
            ("lanes", 5.0),
            ("segs", 88.0),
            ("ghostN", 0.0),
            ("faceOn", 1.0),
            ("rBase", 1.1),
            ("rDepth", 1.7),
            ("rsPow", 0.6),
            ("rMin", 0.3),
        ],
        Mode::Morph => &[("rDot", 0.021), ("iconD", 1.0), ("rMin", 0.25)],
        Mode::Face | Mode::Alert => &[("rMin", 0.25)],
    };
    Opts::from_pairs(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_counts_scale_by_sqrt_and_flat_counts_linearly() {
        let scaled = scale_counts(&base_profile(Mode::Globe), 0.25);
        assert_eq!(scaled.get("latRings", 0.0), 9.0);
        assert_eq!(scaled.get("lonDensity", 0.0), 22.0);
        let orbits = scale_counts(&base_profile(Mode::Orbits), 0.238);
        assert_eq!(orbits.get("orbitN", 0.0), 3.0);
        assert_eq!(orbits.get("ghostN", 0.0), 10.0);
    }

    #[test]
    fn opted_out_layers_stay_out() {
        let ring = scale_counts(&base_profile(Mode::Ring), 0.25);
        assert_eq!(ring.get("ghostN", 9.0), 0.0);
    }

    #[test]
    fn radii_scale_and_remember_the_multiplier() {
        let scaled = scale_radii(&scale_radii(&base_profile(Mode::Orbits), 2.0), 1.2);
        assert!((scaled.get("ghostR", 0.0) - 2.16).abs() < 1e-9);
        assert!((scaled.get("rSizeMul", 0.0) - 2.4).abs() < 1e-9);
    }
}
