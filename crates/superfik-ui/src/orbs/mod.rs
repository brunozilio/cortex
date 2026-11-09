//! Dotted thought-orbs: nine hand-tuned animated states, each a z-sorted
//! field of depth-shaded dots. Pure geometry over `(size, time)`; the
//! `Orb` component paints a frame. A Rust port of Jakub Antalik's
//! `thinking-orbs` (MIT), checked against its golden vectors.

mod alert;
mod blend;
mod braid;
mod core;
mod face;
mod lattice;
mod morph;
mod orbits;
mod presets;
mod profiles;
mod ribbon;
mod web;

pub use blend::blend;
pub use core::{Dot, Line, OrbFrame};
pub use presets::{Mode, OrbSize, OrbState, Resolved, resolve};
pub use profiles::Opts;

pub fn mode_frame(mode: Mode, size: f64, t: f64, opts: &Opts) -> OrbFrame {
    match mode {
        Mode::Orbits => orbits::frame(size, t, opts),
        Mode::Globe => lattice::globe(size, t, opts),
        Mode::Rubik => lattice::rubik(size, t, opts),
        Mode::Wave => lattice::wave(size, t, opts),
        Mode::Web => web::frame(size, t, opts),
        Mode::Braid => braid::frame(size, t, opts),
        Mode::Ribbon | Mode::Ring => ribbon::frame(size, t, opts),
        Mode::Morph => morph::frame(size, t, opts),
        Mode::Face => face::frame(size, t, opts),
        Mode::Alert => alert::frame(size, t, opts),
    }
}

pub fn frame(state: OrbState, size: OrbSize, seconds: f64) -> OrbFrame {
    let resolved = resolve(state, size);
    mode_frame(resolved.mode, size.px(), seconds * resolved.speed, &resolved.opts)
}

#[cfg(test)]
mod golden {
    use super::*;

    const GOLDEN: &str = include_str!("../../tests/fixtures/orbs-golden.json");

    fn state(name: &str) -> OrbState {
        match name {
            "working" => OrbState::Working,
            "searching" => OrbState::Searching,
            "solving" => OrbState::Solving,
            "listening" => OrbState::Listening,
            "connecting" => OrbState::Connecting,
            "weaving" => OrbState::Weaving,
            "composing" => OrbState::Composing,
            "breathing" => OrbState::Breathing,
            "shaping" => OrbState::Shaping,
            other => panic!("unknown state {other}"),
        }
    }

    fn size(px: u64) -> OrbSize {
        match px {
            64 => OrbSize::S64,
            32 => OrbSize::S32,
            _ => OrbSize::S20,
        }
    }

    fn nums(value: &serde_json::Value) -> Vec<f64> {
        value.as_array().expect("array").iter().map(|v| v.as_f64().expect("number")).collect()
    }

    #[test]
    fn frames_match_the_reference_geometry() {
        let golden: serde_json::Value = serde_json::from_str(GOLDEN).expect("fixture");
        let tolerance = golden["tolerance"].as_f64().expect("tolerance");
        for case in golden["cases"].as_array().expect("cases") {
            let key = format!("{}-{}", case["state"], case["size"]);
            let resolved =
                resolve(state(case["state"].as_str().expect("state")), size(case["size"].as_u64().expect("size")));
            let t = case["t"].as_f64().expect("t");
            let frame = mode_frame(resolved.mode, case["size"].as_f64().expect("size"), t, &resolved.opts);
            assert_eq!(frame.dots.len() as u64, case["dotCount"].as_u64().expect("count"), "{key} dot count");
            assert_eq!(frame.lines.len() as u64, case["lineCount"].as_u64().expect("count"), "{key} line count");
            let dots = nums(&case["dots"]);
            for (i, expected) in dots.chunks(6).enumerate() {
                let d = frame.dots[i];
                let actual = [d.x, d.y, d.z, d.r, d.white, d.a];
                for (field, (a, e)) in actual.iter().zip(expected).enumerate() {
                    assert!((a - e).abs() <= tolerance, "{key} dot {i} field {field}: {a} vs {e}");
                }
            }
            let lines = nums(&case["lines"]);
            for (i, expected) in lines.chunks(7).enumerate() {
                let l = frame.lines[i];
                let actual = [l.x1, l.y1, l.x2, l.y2, l.white, l.a, l.w];
                for (field, (a, e)) in actual.iter().zip(expected).enumerate() {
                    assert!((a - e).abs() <= tolerance, "{key} line {i} field {field}: {a} vs {e}");
                }
            }
        }
    }

    #[test]
    fn every_state_renders_at_every_size() {
        for state in OrbState::ALL {
            for size in [OrbSize::S64, OrbSize::S32, OrbSize::S20] {
                assert!(!frame(state, size, 1.0).dots.is_empty(), "{state:?} {size:?}");
            }
        }
    }
}
