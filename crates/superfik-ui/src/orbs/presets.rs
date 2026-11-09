//! The shipped tunings: nine states × three sizes. `count` and `size` are
//! multipliers over the base profiles; `speed` multiplies the clock.

use super::profiles::{Opts, base_profile, scale_counts, scale_radii};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrbState {
    Working,
    Searching,
    Solving,
    Listening,
    Connecting,
    Weaving,
    Composing,
    Breathing,
    Shaping,
    Idle,
    Alert,
}

impl OrbState {
    pub const ALL: [OrbState; 11] = [
        Self::Working,
        Self::Searching,
        Self::Solving,
        Self::Listening,
        Self::Connecting,
        Self::Weaving,
        Self::Composing,
        Self::Breathing,
        Self::Shaping,
        Self::Idle,
        Self::Alert,
    ];

    pub fn mode(self) -> Mode {
        match self {
            Self::Working => Mode::Orbits,
            Self::Searching => Mode::Globe,
            Self::Solving => Mode::Rubik,
            Self::Listening => Mode::Wave,
            Self::Connecting => Mode::Web,
            Self::Weaving => Mode::Braid,
            Self::Composing => Mode::Ribbon,
            Self::Breathing => Mode::Ring,
            Self::Shaping => Mode::Morph,
            Self::Idle => Mode::Face,
            Self::Alert => Mode::Alert,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrbSize {
    S64,
    S32,
    S20,
}

impl OrbSize {
    pub fn px(self) -> f64 {
        match self {
            Self::S64 => 64.0,
            Self::S32 => 32.0,
            Self::S20 => 20.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Orbits,
    Globe,
    Rubik,
    Wave,
    Web,
    Braid,
    Ribbon,
    Ring,
    Morph,
    Face,
    Alert,
}

struct Preset {
    speed: f64,
    count: f64,
    size: f64,
    extra: &'static [(&'static str, f64)],
}

const fn preset(speed: f64, count: f64, size: f64, extra: &'static [(&'static str, f64)]) -> Preset {
    Preset { speed, count, size, extra }
}

fn preset_for(mode: Mode, size: OrbSize) -> Preset {
    use OrbSize::{S20, S32, S64};
    const GLOBE_64: &[(&str, f64)] = &[("scanMul", 4.08), ("dimBase", 0.45)];
    const GLOBE_32: &[(&str, f64)] = &[("scanMul", 4.2301), ("dimBase", 0.45)];
    const GLOBE_20: &[(&str, f64)] = &[("scanMul", 4.335), ("dimBase", 0.45)];
    const RIBBON_64: &[(&str, f64)] = &[("spin", 0.0), ("bandMul", 3.9), ("wobMul", 1.0)];
    const RIBBON_32: &[(&str, f64)] = &[("spin", 0.0), ("bandMul", 4.49), ("wobMul", 1.0)];
    const RIBBON_20: &[(&str, f64)] = &[("spin", 0.0), ("bandMul", 4.94), ("wobMul", 1.0)];
    const RING_64: &[(&str, f64)] = &[("spin", 0.0), ("bandMul", 3.627), ("wobMul", 0.368)];
    const RING_32: &[(&str, f64)] = &[("spin", 0.0), ("bandMul", 3.8265), ("wobMul", 0.4751)];
    const RING_20: &[(&str, f64)] = &[("spin", 0.0), ("bandMul", 3.968), ("wobMul", 0.565)];
    const MORPH: &[(&str, f64)] = &[("spread", 1.45)];
    match (mode, size) {
        (Mode::Orbits, S64) => preset(1.885, 1.0, 1.0, &[]),
        (Mode::Orbits, S32) => preset(2.9072, 0.4251, 1.6849, &[]),
        (Mode::Orbits, S20) => preset(3.9, 0.238, 2.4, &[]),
        (Mode::Globe, S64) => preset(2.015, 0.42, 1.15, GLOBE_64),
        (Mode::Globe, S32) => preset(2.3803, 0.1839, 1.4769, GLOBE_32),
        (Mode::Globe, S20) => preset(2.665, 0.105, 1.75, GLOBE_20),
        (Mode::Rubik, S64) => preset(1.82, 0.35, 1.05, &[]),
        (Mode::Rubik, S32) => preset(1.8964, 0.1537, 1.4951, &[]),
        (Mode::Rubik, S20) => preset(1.95, 0.088, 1.9, &[]),
        (Mode::Wave, S64) => preset(4.388, 0.341, 1.0, &[]),
        (Mode::Wave, S32) => preset(4.1512, 0.169, 1.3232, &[]),
        (Mode::Wave, S20) => preset(3.998, 0.105, 1.6, &[]),
        (Mode::Web, S64) => preset(3.315, 1.35, 0.95, &[]),
        (Mode::Web, S32) => preset(5.0104, 0.4942, 1.2571, &[]),
        (Mode::Web, S20) => preset(6.63, 0.25, 1.52, &[]),
        (Mode::Braid, S64) => preset(1.625, 0.5, 1.0, &[]),
        (Mode::Braid, S32) => preset(2.2234, 0.2056, 1.2011, &[]),
        (Mode::Braid, S20) => preset(2.75, 0.1125, 1.36, &[]),
        (Mode::Ribbon, S64) => preset(2.34, 0.25, 0.85, RIBBON_64),
        (Mode::Ribbon, S32) => preset(2.7776, 0.0969, 0.9766, RIBBON_32),
        (Mode::Ribbon, S20) => preset(3.12, 0.051, 1.073, RIBBON_20),
        (Mode::Ring, S64) => preset(3.24, 0.25, 0.956, RING_64),
        (Mode::Ring, S32) => preset(3.5517, 0.0678, 1.31, RING_32),
        (Mode::Ring, S20) => preset(3.78, 0.028, 1.622, RING_20),
        (Mode::Morph, S64) => preset(2.405, 0.702, 0.395, MORPH),
        (Mode::Morph, S32) => preset(2.2057, 0.5937, 0.6916, MORPH),
        (Mode::Morph, S20) => preset(2.08, 0.53, 1.011, MORPH),
        (Mode::Face | Mode::Alert, S64) => preset(1.0, 1.0, 0.7, &[]),
        (Mode::Face | Mode::Alert, S32) => preset(1.0, 1.0, 0.85, &[]),
        (Mode::Face | Mode::Alert, S20) => preset(1.0, 1.0, 1.0, &[]),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub mode: Mode,
    pub speed: f64,
    pub opts: Opts,
}

pub fn resolve(state: OrbState, size: OrbSize) -> Resolved {
    let mode = state.mode();
    let preset = preset_for(mode, size);
    let mut opts = base_profile(mode);
    if preset.count != 1.0 {
        opts = scale_counts(&opts, preset.count);
    }
    if preset.size != 1.0 {
        opts = scale_radii(&opts, preset.size);
    }
    for (key, value) in preset.extra {
        opts.set(key, *value);
    }
    Resolved { mode, speed: preset.speed, opts }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_at_20_matches_the_reference_resolution() {
        let r = resolve(OrbState::Working, OrbSize::S20);
        assert_eq!(r.mode, Mode::Orbits);
        assert_eq!(r.speed, 3.9);
        assert_eq!(r.opts.get("orbitN", 0.0), 3.0);
        assert_eq!(r.opts.get("ghostN", 0.0), 10.0);
        assert!((r.opts.get("ghostR", 0.0) - 2.16).abs() < 1e-9);
        assert!((r.opts.get("partRDepth", 0.0) - 3.84).abs() < 1e-9);
        assert!((r.opts.get("rSizeMul", 0.0) - 2.4).abs() < 1e-9);
    }

    #[test]
    fn extras_land_last() {
        let r = resolve(OrbState::Breathing, OrbSize::S64);
        assert_eq!(r.opts.get("wobMul", 0.0), 0.368);
        assert_eq!(r.opts.get("ghostN", 1.0), 0.0);
    }
}
