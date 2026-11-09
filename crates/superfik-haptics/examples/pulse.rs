//! Calibrating the trackpad by feel: `cargo run -p superfik-haptics --example pulse`.
//! Plays numbered candidates two seconds apart (rest a finger on the
//! trackpad), then the app's own five haptics.

use std::time::Duration;

use superfik_haptics::{Actuator, Haptic, Strike, Trackpad};

const BURST: f64 = 0.02;

fn main() {
    let trackpad = Trackpad::open(BURST);
    let mut candidates: Vec<(String, Strike)> =
        [1, 2, 3, 4, 5, 6, 15, 16].map(|a| (format!("waveform {a}"), Strike { actuation: a, repeats: 1 })).into();
    for repeats in [2, 3, 4] {
        candidates.push((format!("waveform 6 × {repeats}"), Strike { actuation: 6, repeats }));
    }
    for (n, (label, strike)) in candidates.iter().enumerate() {
        println!("{:>2}. {label}", n + 1);
        if !trackpad.strike(*strike) {
            println!("    (no actuator: only AppKit patterns are available)");
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    println!("--- the app's haptics");
    for haptic in Haptic::ALL {
        println!("{haptic:?} {:?}", haptic.strike());
        trackpad.perform(haptic);
        std::thread::sleep(Duration::from_secs(2));
    }
}
