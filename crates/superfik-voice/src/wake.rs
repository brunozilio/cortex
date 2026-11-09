//! The wake word: a word the user says to start talking to the agent
//! ("Piratinha" unless changed), and finding it in what was heard. Pure.

use serde::{Deserialize, Serialize};

pub const WAKE_WORD_BYTES: usize = crate::short_text::SHORT_TEXT_BYTES;

crate::short_text::short_text! {
    WakeWord = "Piratinha"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WakePrefs {
    pub enabled: bool,
    pub word: WakeWord,
}

pub fn find(heard: &str, word: WakeWord) -> Option<String> {
    let target: String = word.as_str().split_whitespace().map(fold).collect();
    if target.is_empty() {
        return None;
    }
    let allowed = if target.chars().count() < 5 { 0 } else { (target.chars().count() / 8).max(1) };
    let tokens: Vec<&str> = heard.split_whitespace().collect();
    for start in 0..tokens.len() {
        let mut said = String::new();
        let mut best: Option<(usize, usize)> = None;
        for (span, token) in tokens[start..].iter().take(SPAN.max(word_count(word))).enumerate() {
            said.push_str(&fold(token));
            let distance = edits(&said, &target);
            if distance <= allowed && best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, start + span));
            }
        }
        if let Some((_, last)) = best {
            let end = offset(heard, tokens[last]) + tokens[last].len();
            let rest = heard[end..].trim_start_matches(|c: char| c.is_whitespace() || is_mark(c));
            return Some(rest.trim_end().to_owned());
        }
    }
    None
}

const SPAN: usize = 2;

fn word_count(word: WakeWord) -> usize {
    word.as_str().split_whitespace().count() * SPAN
}

fn offset(heard: &str, token: &str) -> usize {
    token.as_ptr() as usize - heard.as_ptr() as usize
}

fn is_mark(c: char) -> bool {
    matches!(c, ',' | '.' | '!' | '?' | ';' | ':' | '-' | '—' | '–' | '…')
}

fn fold(token: &str) -> String {
    token
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1).min(row[j] + 1).min(diagonal + usize::from(ca != *cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piratinha() -> WakeWord {
        WakeWord::default()
    }

    #[test]
    fn the_default_word_is_piratinha() {
        assert_eq!(WakeWord::default().as_str(), "Piratinha");
    }

    #[test]
    fn a_word_is_trimmed_cut_to_fit_and_never_empty() {
        assert_eq!(WakeWord::new("  Jarvis ").map(|w| w.as_str().to_owned()), Some("Jarvis".to_owned()));
        assert_eq!(WakeWord::new("   "), None);
        let long = "ã".repeat(40);
        let cut = WakeWord::new(&long).expect("fits");
        assert_eq!(cut.as_str().chars().count(), WAKE_WORD_BYTES / 2);
    }

    #[test]
    fn it_serialises_as_text_and_an_empty_one_reads_back_as_the_default() {
        let jarvis = WakeWord::new("Jarvis").expect("word");
        assert_eq!(serde_json::to_string(&jarvis).expect("json"), r#""Jarvis""#);
        let back: WakeWord = serde_json::from_str(r#""Jarvis""#).expect("parses");
        assert_eq!(back, jarvis);
        let empty: WakeWord = serde_json::from_str(r#""""#).expect("parses");
        assert_eq!(empty, WakeWord::default());
    }

    #[test]
    fn the_word_alone_wakes_with_nothing_after_it() {
        assert_eq!(find("Piratinha", piratinha()), Some(String::new()));
        assert_eq!(find("piratinha.", piratinha()), Some(String::new()));
    }

    #[test]
    fn what_follows_the_word_is_kept_as_said() {
        assert_eq!(find("Piratinha, roda os testes.", piratinha()), Some("roda os testes.".to_owned()));
        assert_eq!(find("ei PIRATINHA! Que horas são?", piratinha()), Some("Que horas são?".to_owned()));
    }

    #[test]
    fn accents_a_close_spelling_and_a_split_word_still_wake() {
        assert_eq!(find("Piratínha abre o PR", piratinha()), Some("abre o PR".to_owned()));
        assert_eq!(find("piratinho abre o PR", piratinha()), Some("abre o PR".to_owned()));
        assert_eq!(find("pira tinha abre o PR", piratinha()), Some("abre o PR".to_owned()));
    }

    #[test]
    fn other_words_do_not_wake() {
        for heard in ["pirata", "piranha", "a pirataria", "roda os testes", ""] {
            assert_eq!(find(heard, piratinha()), None, "{heard}");
        }
    }

    #[test]
    fn a_short_word_must_be_said_exactly() {
        let ei = WakeWord::new("Ola").expect("word");
        assert_eq!(find("olá tudo bem", ei), Some("tudo bem".to_owned()));
        assert_eq!(find("ala tudo bem", ei), None);
    }

    #[test]
    fn a_word_of_two_words_wakes_when_both_are_said() {
        let ei = WakeWord::new("Ei Superfik").expect("word");
        assert_eq!(find("ei superfik, abre o canvas", ei), Some("abre o canvas".to_owned()));
        assert_eq!(find("superfik abre o canvas", ei), None);
    }
}
