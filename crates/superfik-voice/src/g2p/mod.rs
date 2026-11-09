//! Brazilian Portuguese text into Kokoro's phonemes.
//!
//! Kokoro reads phonemes, not letters. Its Portuguese voices were trained
//! on espeak-ng's, but espeak-ng is GPL (and not bundled by sherpa-onnx), so
//! the app spells Portuguese out itself: numbers and acronyms become words
//! (`numbers`), each word becomes IPA by rule (`word`), punctuation stays as
//! Kokoro's pauses. Pure.

mod numbers;
mod syllables;
mod word;

pub use numbers::{cardinal, spoken};
pub use word::ipa;

pub const KOKORO_SYMBOLS: &str = ";:,.!?—…\"()“” \u{303}ʣʥʦʨᵝꭧAIOQSTWYᵊabcdefhijklmnopqrstuvwxyzɑɐɒæβɔɕçɖðʤəɚɛɜɟɡɥɨɪʝɯɰŋɳɲɴøɸθœɹɾɻʁɽʂʃʈʧʊʋʌɣɤχʎʒʔˈˌːʰʲ↓→↗↘ᵻ";

pub fn codepoints(sentence: &str) -> Vec<i32> {
    phonemes(sentence).chars().map(|c| c as i32).collect()
}

pub fn phonemes(sentence: &str) -> String {
    let mut out = String::new();
    for token in tokens(sentence) {
        match token {
            Token::Word(w) => {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(&ipa(&w));
            }
            Token::Pause(p) => {
                let trimmed = out.trim_end().len();
                out.truncate(trimmed);
                if !out.is_empty() && !out.ends_with([',', '.', '!', '?']) {
                    out.push(p);
                }
                out.push(' ');
            }
        }
    }
    out.retain(|c| KOKORO_SYMBOLS.contains(c));
    out.trim().to_owned()
}

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Pause(char),
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for raw in text.split_whitespace() {
        let trimmed = raw.trim_matches(|c: char| !c.is_alphanumeric());
        let lead = &raw[..raw.find(trimmed).unwrap_or(0)];
        if lead.contains(['(', '—', '–']) {
            out.push(Token::Pause(','));
        }
        say(trimmed, &mut out);
        let trail = &raw[raw.find(trimmed).map_or(raw.len(), |at| at + trimmed.len())..];
        if trail.contains('%') {
            words("por cento", &mut out);
        }
        if let Some(pause) = trail.chars().rev().find_map(pause) {
            out.push(Token::Pause(pause));
        } else if raw.chars().all(|c| matches!(c, '—' | '–' | '-')) && !raw.is_empty() {
            out.push(Token::Pause(','));
        }
    }
    out
}

fn pause(c: char) -> Option<char> {
    match c {
        '.' | '…' => Some('.'),
        '!' => Some('!'),
        '?' => Some('?'),
        ',' | ';' | ':' | ')' | '—' | '–' => Some(','),
        _ => None,
    }
}

fn say(piece: &str, out: &mut Vec<Token>) {
    if piece.is_empty() {
        return;
    }
    if let Some(number) = spoken(piece) {
        return words(&number, out);
    }
    for part in piece.split(|c: char| !(c.is_alphanumeric() || matches!(c, ',' | '.'))) {
        let part = part.trim_matches([',', '.']);
        if part.is_empty() {
            continue;
        }
        if let Some(number) = spoken(part) {
            words(&number, out);
        } else if part.chars().any(|c| c.is_ascii_digit()) {
            let mut run = String::new();
            for c in part.chars() {
                if !run.is_empty() && run.chars().all(|r| r.is_ascii_digit()) != c.is_ascii_digit() {
                    say(&std::mem::take(&mut run), out);
                }
                run.push(c);
            }
            say(&run, out);
        } else if is_acronym(part) || is_lone_consonant(part) {
            for c in part.chars() {
                out.push(Token::Word(numbers::letter(c).into()));
            }
        } else {
            out.push(Token::Word(part.into()));
        }
    }
}

fn words(text: &str, out: &mut Vec<Token>) {
    out.extend(text.split_whitespace().map(|w| Token::Word(w.into())));
}

fn is_acronym(word: &str) -> bool {
    let n = word.chars().count();
    (2..=5).contains(&n) && word.chars().all(|c| c.is_ascii_uppercase())
}

fn is_lone_consonant(word: &str) -> bool {
    let mut chars = word.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphabetic() && !syllables::is_vowel(c.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sentence_keeps_its_pauses() {
        let decomposed = word::tests::decomposed;
        assert_eq!(phonemes("Olá, tudo bem?"), decomposed("olˈa, tˈudʊ bˈẽj̃?"));
        assert_eq!(phonemes("Pronto. Abri o PR."), decomposed("pɾˈõtʊ. abɾˈi ʊ pˈe ˈɛxi."));
    }

    #[test]
    fn numbers_and_symbols_are_spelled_out() {
        assert_eq!(phonemes("3 testes"), phonemes("três testes"));
        assert_eq!(phonemes("subiu 5%"), phonemes("subiu cinco por cento"));
        assert_eq!(phonemes("versão v2"), phonemes("versão vê dois"));
    }

    #[test]
    fn nothing_kokoro_cannot_read_gets_through() {
        let said = phonemes("Código «x» → feito ✅ #42 @ana");
        assert!(said.chars().all(|c| KOKORO_SYMBOLS.contains(c)), "{said}");
        assert!(!said.is_empty());
    }

    #[test]
    fn codepoints_are_the_phonemes() {
        let points = codepoints("voz");
        assert_eq!(points.iter().map(|&c| char::from_u32(c as u32).expect("test")).collect::<String>(), "vˈos");
    }
}
