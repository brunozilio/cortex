//! The models the voice agent runs: which speech recognition checkpoint
//! (NVIDIA Nemotron, streaming), in which language, and which Kokoro voice
//! speaks. Pure: names, download locations and capabilities.
//!
//! Recognition runs the sherpa-onnx int8 exports of NVIDIA's checkpoints.
//! `nemotron-3.5-asr-streaming-0.6b` is multilingual (the target language is
//! given as a prompt) and the only one that understands Portuguese, so it is
//! the default; `nemotron-speech-streaming-en-0.6b` is English only. A smaller
//! chunk size answers sooner and transcribes a little worse.

use serde::{Deserialize, Serialize};

const RELEASES: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chunk {
    #[serde(rename = "80ms")]
    Ms80,
    #[serde(rename = "160ms")]
    Ms160,
    #[serde(rename = "560ms")]
    Ms560,
    #[serde(rename = "1120ms")]
    Ms1120,
}

impl Chunk {
    pub const ALL: [Self; 4] = [Self::Ms80, Self::Ms160, Self::Ms560, Self::Ms1120];

    pub const fn tag(self) -> &'static str {
        match self {
            Self::Ms80 => "80ms",
            Self::Ms160 => "160ms",
            Self::Ms560 => "560ms",
            Self::Ms1120 => "1120ms",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "family", content = "chunk", rename_all = "kebab-case")]
pub enum Recognizer {
    Nemotron35(Chunk),
    NemotronEnglish(Chunk),
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::Nemotron35(Chunk::Ms560)
    }
}

impl Recognizer {
    pub const ALL: [Self; 8] = [
        Self::Nemotron35(Chunk::Ms80),
        Self::Nemotron35(Chunk::Ms160),
        Self::Nemotron35(Chunk::Ms560),
        Self::Nemotron35(Chunk::Ms1120),
        Self::NemotronEnglish(Chunk::Ms80),
        Self::NemotronEnglish(Chunk::Ms160),
        Self::NemotronEnglish(Chunk::Ms560),
        Self::NemotronEnglish(Chunk::Ms1120),
    ];

    pub const fn chunk(self) -> Chunk {
        match self {
            Self::Nemotron35(chunk) | Self::NemotronEnglish(chunk) => chunk,
        }
    }

    pub const fn with_chunk(self, chunk: Chunk) -> Self {
        match self {
            Self::Nemotron35(_) => Self::Nemotron35(chunk),
            Self::NemotronEnglish(_) => Self::NemotronEnglish(chunk),
        }
    }

    pub const fn checkpoint(self) -> &'static str {
        match self {
            Self::Nemotron35(_) => "nvidia/nemotron-3.5-asr-streaming-0.6b",
            Self::NemotronEnglish(_) => "nvidia/nemotron-speech-streaming-en-0.6b",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Nemotron35(_) => "Nemotron 3.5",
            Self::NemotronEnglish(_) => "Nemotron English",
        }
    }

    pub fn folder(self) -> String {
        let chunk = self.chunk().tag();
        match self {
            Self::Nemotron35(_) => format!("sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-{chunk}-int8-2026-06-11"),
            Self::NemotronEnglish(_) => {
                format!("sherpa-onnx-nemotron-speech-streaming-en-0.6b-{chunk}-int8-2026-04-25")
            }
        }
    }

    pub fn url(self) -> String {
        format!("{RELEASES}/asr-models/{}.tar.bz2", self.folder())
    }

    pub const fn understands(self, language: Language) -> bool {
        match self {
            Self::Nemotron35(_) => true,
            Self::NemotronEnglish(_) => matches!(language, Language::English),
        }
    }

    pub const fn prompt(self, language: Language) -> Option<&'static str> {
        match self {
            Self::Nemotron35(_) => Some(language.locale()),
            Self::NemotronEnglish(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "pt-BR")]
    PortugueseBrazil,
    #[serde(rename = "en-US")]
    English,
    #[serde(rename = "auto")]
    Auto,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::PortugueseBrazil, Self::English, Self::Auto];

    pub const fn locale(self) -> &'static str {
        match self {
            Self::PortugueseBrazil => "pt-BR",
            Self::English => "en-US",
            Self::Auto => "auto",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Voice {
    #[default]
    PmSanta,
    PmAlex,
    PfDora,
    AfHeart,
    AmMichael,
}

impl Voice {
    pub const ALL: [Self; 5] = [Self::PmSanta, Self::PmAlex, Self::PfDora, Self::AfHeart, Self::AmMichael];

    pub const fn name(self) -> &'static str {
        match self {
            Self::PmSanta => "pm_santa",
            Self::PmAlex => "pm_alex",
            Self::PfDora => "pf_dora",
            Self::AfHeart => "af_heart",
            Self::AmMichael => "am_michael",
        }
    }

    pub const fn speaker(self) -> i32 {
        match self {
            Self::AfHeart => 3,
            Self::AmMichael => 16,
            Self::PfDora => 42,
            Self::PmAlex => 43,
            Self::PmSanta => 44,
        }
    }

    pub const fn speaks(self) -> Spoken {
        match self {
            Self::PmSanta | Self::PmAlex | Self::PfDora => Spoken::Portuguese,
            Self::AfHeart | Self::AmMichael => Spoken::English,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spoken {
    Portuguese,
    English,
}

pub const KOKORO_FOLDER: &str = "kokoro-multi-lang-v1_0";

pub fn kokoro_url() -> String {
    format!("{RELEASES}/tts-models/{KOKORO_FOLDER}.tar.bz2")
}

pub const VAD_FILE: &str = "silero_vad.onnx";

pub fn vad_url() -> String {
    format!("{RELEASES}/asr-models/{VAD_FILE}")
}

pub const KOKORO_RATE: u32 = 24_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_checkpoint_understands_brazilian_portuguese() {
        let recognizer = Recognizer::default();
        assert_eq!(recognizer.checkpoint(), "nvidia/nemotron-3.5-asr-streaming-0.6b");
        assert!(recognizer.understands(Language::PortugueseBrazil));
        assert_eq!(recognizer.prompt(Language::default()), Some("pt-BR"));
        assert_eq!(
            recognizer.url(),
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/\
             sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11.tar.bz2"
        );
    }

    #[test]
    fn the_english_checkpoint_takes_no_language_and_no_portuguese() {
        let english = Recognizer::NemotronEnglish(Chunk::Ms160);
        assert!(!english.understands(Language::PortugueseBrazil));
        assert!(english.understands(Language::English));
        assert_eq!(english.prompt(Language::English), None);
        assert_eq!(english.folder(), "sherpa-onnx-nemotron-speech-streaming-en-0.6b-160ms-int8-2026-04-25");
    }

    #[test]
    fn changing_the_chunk_keeps_the_family() {
        assert_eq!(Recognizer::default().with_chunk(Chunk::Ms80), Recognizer::Nemotron35(Chunk::Ms80));
        assert_eq!(Recognizer::ALL.iter().filter(|r| r.understands(Language::PortugueseBrazil)).count(), 4);
    }

    #[test]
    fn santa_is_the_default_voice_and_speaks_portuguese() {
        assert_eq!(Voice::default().name(), "pm_santa");
        assert_eq!(Voice::default().speaker(), 44);
        assert_eq!(Voice::default().speaks(), Spoken::Portuguese);
    }

    #[test]
    fn choices_keep_their_names_when_saved() {
        let json =
            serde_json::to_string(&(Recognizer::default(), Language::default(), Voice::default())).expect("test");
        assert_eq!(json, r#"[{"family":"nemotron35","chunk":"560ms"},"pt-BR","pm_santa"]"#);
        let back: (Recognizer, Language, Voice) = serde_json::from_str(&json).expect("test");
        assert_eq!(back, (Recognizer::default(), Language::default(), Voice::default()));
    }
}
