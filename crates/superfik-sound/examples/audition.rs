//! Plays every cue of a pack, one after another, to compare voices:
//! `cargo run -p superfik-sound --example audition -- sine`

use std::time::Duration;

use superfik_sound::{Cue, PackId, SoundPrefs, Sounds};

fn main() {
    tracing_subscriber::fmt().with_env_filter("superfik_sound=trace,warn").init();
    let pack = std::env::args().nth(1).map(|s| s.parse::<PackId>()).transpose();
    let pack = match pack {
        Ok(pack) => pack.unwrap_or_default(),
        Err(error) => {
            eprintln!("{error}; packs: {}", PackId::ALL.map(PackId::name).join(", "));
            std::process::exit(2);
        }
    };
    let mut sounds = Sounds::open(SoundPrefs { enabled: true, pack });
    println!("pack: {pack}");
    for cue in Cue::ALL {
        println!("  {}", cue.name());
        sounds.play(cue);
        std::thread::sleep(Duration::from_millis(900));
    }
    std::thread::sleep(Duration::from_millis(1500));
}
