//! Tries the voice agent's models from the terminal.
//!
//! ```text
//! cargo run -p superfik-voice --features native --example talk -- say "Olá, tudo bem?"
//! cargo run -p superfik-voice --features native --example talk -- roundtrip
//! cargo run -p superfik-voice --features native --example talk -- listen
//! cargo run -p superfik-voice --features native --example talk -- wake [word]
//! cargo run -p superfik-voice --features native --example talk -- watch [word]
//! cargo run -p superfik-voice --features native --example talk -- watchfeed
//! ```
//!
//! `say` speaks with pm_santa; `roundtrip` speaks a set of Portuguese
//! sentences and has Nemotron transcribe them back (how well our phonemes
//! read); `listen` prints what the microphone hears until Ctrl-C; `wake`
//! has the Portuguese voices say phrases with the wake word and shows
//! whether it is found in what Nemotron heard; `watch` waits for it on
//! the microphone and prints each `Woke`; `watchfeed` feeds a spoken
//! wake phrase between two seconds of silence, at 48 kHz, through the
//! wake-word watcher's phrase detector and prints the phrases it closed
//! (the watcher without a microphone). Models
//! are fetched into `SUPERFIK_VOICE_MODELS` (default: the app's folder).

use std::path::PathBuf;
use std::sync::Arc;

use superfik_voice::native::{LocalVoice, Models, synthesize, transcribe};
use superfik_voice::{KOKORO_RATE, Voice, VoiceCall, VoiceDevice, VoiceEvent, VoicePrefs, WakeWord, g2p};

const SENTENCES: &[&str] = &[
    "Olá, tudo bem? Quero criar um agente de voz.",
    "Encontrei o problema no arquivo de configuração e já corrigi.",
    "Os testes passaram, mas o clippy reclamou de duas funções.",
    "Abri o pull request com as mudanças na interface.",
    "Amanhã de manhã eu reviso a documentação com calma.",
    "A cidade tem muitas árvores e o coração bate mais forte.",
    "São três horas e quarenta minutos.",
];

const WAKE_SENTENCES: &[&str] =
    &["{}", "{}, roda os testes.", "Ei {}, que horas são?", "{} abre o pull request", "O pirata tinha um navio."];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let root = std::env::var_os("SUPERFIK_VOICE_MODELS").map(PathBuf::from).unwrap_or_else(default_root);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let prefs = VoicePrefs::default();
    fetch(&root, prefs)?;
    match args.first().map(String::as_str) {
        Some("say") => {
            let text = args.get(1).map_or(SENTENCES[0], String::as_str);
            println!("{}", g2p::phonemes(text));
            let samples = synthesize(root, text, Voice::PmSanta)?;
            write_wav("santa.wav", &samples)?;
            println!("santa.wav");
        }
        Some("roundtrip") => {
            for sentence in SENTENCES {
                let samples = synthesize(root.clone(), sentence, Voice::PmSanta)?;
                let heard = transcribe(root.clone(), prefs, &samples, KOKORO_RATE as i32)?;
                println!("{sentence}\n  {}\n  → {heard}\n", g2p::phonemes(sentence));
            }
        }
        Some("listen") => {
            let voice = LocalVoice::new(root, Arc::new(|| {}));
            voice.call(VoiceCall::Listen(prefs));
            loop {
                for event in voice.poll() {
                    match event {
                        VoiceEvent::Heard { text, ended: true } => println!("✓ {text}"),
                        VoiceEvent::Heard { text, .. } => eprint!("\r… {text}   "),
                        other => println!("{other:?}"),
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        Some("wake") => {
            let word = superfik_voice::WakeWord::new(args.get(1).map_or(WakeWord::DEFAULT, String::as_str))
                .unwrap_or_default();
            for voice in [Voice::PmSanta, Voice::PmAlex, Voice::PfDora] {
                for sentence in WAKE_SENTENCES {
                    let sentence = sentence.replace("{}", word.as_str());
                    let samples = synthesize(root.clone(), &sentence, voice)?;
                    let heard = transcribe(root.clone(), prefs, &samples, KOKORO_RATE as i32)?;
                    let found = superfik_voice::wake::find(&heard, word);
                    println!(
                        "{voice:?}: {sentence}\n  → {heard}\n  {}\n",
                        found.map_or("✗".into(), |rest| format!("✓ «{rest}»"))
                    );
                }
            }
        }
        Some("watchfeed") => {
            let spoken = synthesize(root.clone(), "Piratinha, roda os testes.", Voice::PmSanta)?;
            let up = sherpa_onnx::LinearResampler::create(KOKORO_RATE as i32, 48_000).ok_or("resampler")?;
            let mut samples = vec![0.0; 96_000];
            samples.extend(up.resample(&spoken, true));
            samples.extend(vec![0.0; 96_000]);
            let heard = superfik_voice::native::watch_phrases(root, prefs, &samples, 48_000)?;
            println!("phrases: {heard:?}");
        }
        Some("watch") => {
            let word = WakeWord::new(args.get(1).map_or(WakeWord::DEFAULT, String::as_str)).unwrap_or_default();
            let voice = LocalVoice::new(root, Arc::new(|| {}));
            voice.call(VoiceCall::Watch { prefs, word });
            eprintln!("say «{}»…", word.as_str());
            loop {
                for event in voice.poll() {
                    match event {
                        VoiceEvent::Woke { rest } => println!("✓ woke: «{rest}»"),
                        other => println!("{other:?}"),
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        _ => eprintln!("usage: talk say [text] | roundtrip | listen | wake [word] | watch [word] | watchfeed"),
    }
    Ok(())
}

fn default_root() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/Superfik/models/voice")
}

fn fetch(root: &std::path::Path, prefs: VoicePrefs) -> Result<(), superfik_voice::VoiceProblem> {
    let models = Models::new(root.to_owned());
    let show = |what: &'static str| move |f: f32| eprint!("\r{what} {:>3.0}%", f * 100.0);
    models.fetch_recognizer(prefs.recognizer, show("Nemotron"))?;
    models.fetch_kokoro(show("Kokoro"))?;
    models.fetch_vad()?;
    eprintln!();
    Ok(())
}

fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    let data: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
    let mut out = Vec::with_capacity(44 + data.len());
    let size = |n: usize| (n as u32).to_le_bytes();
    out.extend(b"RIFF");
    out.extend(size(36 + data.len()));
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(KOKORO_RATE.to_le_bytes());
    out.extend((KOKORO_RATE * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(size(data.len()));
    out.extend(data);
    std::fs::write(path, out)
}
