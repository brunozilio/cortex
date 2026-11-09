//! One Brazilian Portuguese word, spelled, into the IPA Kokoro was trained
//! on (espeak-ng's `pt-br`: `obɾiɡˈadʊ`, `kõputadˈoɾ`, `ʧˈia`). Pure rules:
//! the stress from the spelling, nasal vowels, reduced final vowels, `t`
//! and `d` softened before `i`, the open `r`, `l` closing a syllable as
//! `w`. Words it cannot know (loans, open or closed `e`/`o` without an
//! accent) come out close enough to be understood.

use super::syllables::{is_glide_u, is_silent_u, is_vowel, nuclei, stressed};

const STRESS: &str = "ˈ";
const TILDE: char = '\u{303}';

pub fn ipa(word: &str) -> String {
    let w: Vec<char> = word.to_lowercase().chars().collect();
    let nuclei = nuclei(&w);
    let stress = stressed(&w, &nuclei);
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < w.len() {
        let c = w[i];
        let at = nuclei.iter().position(|&(s, e)| (s..e).contains(&i));
        if let Some(n) = at {
            let (start, end) = nuclei[n];
            if i == start {
                if stress == Some(n) {
                    out.push(STRESS.into());
                }
                let (sound, used) = nucleus(&w, start, end, Some(n) == stress, n + 1 == nuclei.len(), stress);
                out.push(sound);
                i = used;
                continue;
            }
        }
        let next = w.get(i + 1).copied();
        let prev = i.checked_sub(1).map(|p| w[p]);
        let (sound, skip) = consonant(&w, i, c, prev, next);
        out.push(sound.into());
        i += 1 + skip;
    }
    soften(&mut out);
    out.concat()
}

fn nucleus(w: &[char], start: usize, end: usize, stressed: bool, last: bool, stress: Option<usize>) -> (String, usize) {
    let letters: String = w[start..end].iter().collect();
    let after = |k: usize| w.get(end + k).copied();
    let closes = |c: Option<char>, then: Option<char>| {
        matches!(c, Some('m' | 'n')) && then.is_none_or(|t| !is_vowel(t) && t != 'h')
    };
    let nasal = closes(after(0), after(1));
    let used = end + usize::from(nasal);
    let final_s = |k| after(k) == Some('s') && after(k + 1).is_none();
    let at_end = |k: usize| after(k).is_none() || final_s(k);
    let mut sound = match letters.as_str() {
        "ão" => "ɐ̃w̃".into(),
        "ãe" => "ɐ̃j̃".into(),
        "õe" => "o\u{303}j\u{303}".into(),
        "a" if nasal && at_end(1) && !stressed && after(0) == Some('m') => "ɐ̃w̃".into(),
        "e" | "é" | "ê" if nasal && at_end(1) => "e\u{303}j\u{303}".into(),
        _ => {
            let mut s = String::new();
            for (k, &c) in w[start..end].iter().enumerate() {
                let glide = k > 0;
                let reduced = last && !nasal && stress.is_some_and(|_| !stressed);
                s.push_str(&vowel(w, start + k, c, stressed, reduced, glide, nasal));
            }
            if nasal {
                s.push(TILDE);
            }
            s
        }
    };
    if nasal && sound.starts_with('a') {
        sound.replace_range(0..1, "ɐ");
    }
    (sound, used)
}

fn vowel(w: &[char], i: usize, c: char, stressed: bool, reduced_final: bool, glide: bool, nasal: bool) -> String {
    if glide {
        return match c {
            'i' | 'e' => "j",
            'u' | 'o' => "w",
            _ => "",
        }
        .into();
    }
    let nasal_next = w.get(i + 1).is_some_and(|n| matches!(n, 'm' | 'n')) && w.get(i + 2).is_some_and(|n| is_vowel(*n))
        || w.get(i + 1) == Some(&'n') && w.get(i + 2) == Some(&'h');
    let clitic_or_final = reduced_final || !nasal && !stressed && monosyllable(w);
    match c {
        'a' | 'à' if clitic_or_final => "ɐ",
        'a' if stressed && nasal_next => "ɐ",
        'a' | 'á' | 'à' => "a",
        'â' => "ɐ",
        'ã' => "ɐ\u{303}",
        'e' if clitic_or_final => "i",
        'é' => "ɛ",
        'e' | 'ê' => "e",
        'i' | 'í' | 'y' => "i",
        'o' if clitic_or_final => "ʊ",
        'ó' => "ɔ",
        'o' | 'ô' => "o",
        'õ' => "o\u{303}",
        _ => "u",
    }
    .into()
}

fn monosyllable(w: &[char]) -> bool {
    nuclei(w).len() == 1
}

fn consonant(w: &[char], i: usize, c: char, prev: Option<char>, next: Option<char>) -> (&'static str, usize) {
    let front = next.is_some_and(|n| matches!(n, 'e' | 'i' | 'é' | 'ê' | 'í' | 'y'));
    let vowel_next = next.is_some_and(is_vowel);
    let vowel_prev = prev.is_some_and(is_vowel);
    let ends = next.is_none();
    match c {
        'c' if next == Some('h') => ("ʃ", 1),
        'c' if front => ("s", 0),
        'c' | 'k' => ("k", 0),
        'ç' => ("s", 0),
        'g' if front => ("ʒ", 0),
        'g' if is_glide_u(w, i + 1) => ("ɡw", 1),
        'g' => ("ɡ", 0),
        'q' if is_silent_u(w, i + 1) => ("k", 1),
        'q' if is_glide_u(w, i + 1) => ("kw", 1),
        'q' => ("k", 0),
        'u' if is_silent_u(w, i) => ("", 0),
        'h' => ("", 0),
        'j' => ("ʒ", 0),
        'l' if next == Some('h') => ("ʎ", 1),
        'l' if !vowel_next => ("w", 0),
        'n' if next == Some('h') => ("ɲ", 1),
        'r' if next == Some('r') => ("x", 1),
        'r' if prev.is_none() || matches!(prev, Some('n' | 'l' | 's')) => ("x", 0),
        'r' => ("ɾ", 0),
        's' if next == Some('s') => ("s", 1),
        's' if next == Some('c') && w.get(i + 2).is_some_and(|n| matches!(n, 'e' | 'i')) => ("s", 1),
        's' if vowel_prev && vowel_next => ("z", 0),
        's' if next.is_some_and(|n| matches!(n, 'b' | 'd' | 'g' | 'l' | 'm' | 'n' | 'r' | 'v')) => ("z", 0),
        'x' if ends => ("ks", 0),
        'x' if prev == Some('e') && i == 1 && vowel_next => ("z", 0),
        'x' if !vowel_next => ("s", 0),
        'x' => ("ʃ", 0),
        'z' if ends => ("s", 0),
        'w' => ("w", 0),
        'b' => ("b", 0),
        'd' => ("d", 0),
        'f' => ("f", 0),
        'm' => ("m", 0),
        'n' => ("n", 0),
        'p' => ("p", 0),
        's' => ("s", 0),
        't' => ("t", 0),
        'v' => ("v", 0),
        'z' => ("z", 0),
        'l' => ("l", 0),
        _ => ("", 0),
    }
}

fn soften(out: &mut [String]) {
    for k in 0..out.len() {
        let next = out[k + 1..].iter().find(|s| !s.is_empty() && *s != STRESS);
        let before_i = next.is_some_and(|s| s.starts_with('i'));
        match out[k].as_str() {
            "t" if before_i => out[k] = "ʧ".into(),
            "d" if before_i => out[k] = "ʤ".into(),
            _ => {}
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::ipa;

    pub(crate) fn decomposed(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'ã' => "a\u{303}".to_owned(),
                'ẽ' => "e\u{303}".to_owned(),
                'ĩ' => "i\u{303}".to_owned(),
                'õ' => "o\u{303}".to_owned(),
                'ũ' => "u\u{303}".to_owned(),
                other => other.to_string(),
            })
            .collect()
    }

    #[test]
    fn everyday_words() {
        for (word, expected) in [
            ("olá", "olˈa"),
            ("tudo", "tˈudʊ"),
            ("bem", "bˈẽj̃"),
            ("obrigado", "obɾiɡˈadʊ"),
            ("computador", "kõputadˈoɾ"),
            ("cidade", "sidˈaʤi"),
            ("tia", "ʧˈiɐ"),
            ("noite", "nˈojʧi"),
            ("coração", "koɾasˈɐ̃w̃"),
            ("muito", "mˈujtʊ"),
            ("agente", "aʒˈẽʧi"),
            ("de", "ʤi"),
            ("que", "ki"),
            ("voz", "vˈos"),
            ("Brasil", "bɾazˈiw"),
            ("rato", "xˈatʊ"),
            ("carro", "kˈaxʊ"),
            ("filho", "fˈiʎʊ"),
            ("ninho", "nˈiɲʊ"),
            ("chave", "ʃˈavi"),
            ("guerra", "ɡˈexɐ"),
            ("casa", "kˈazɐ"),
            ("falam", "fˈalɐ̃w̃"),
            ("mesmo", "mˈezmʊ"),
            ("texto", "tˈestʊ"),
            ("café", "kafˈɛ"),
            ("manhã", "maɲˈɐ̃"),
            ("quarenta", "kwaɾˈẽtɐ"),
            ("água", "ˈaɡwɐ"),
            ("com", "kõ"),
        ] {
            assert_eq!(ipa(word), decomposed(expected), "{word}");
        }
    }

    #[test]
    fn every_sound_is_one_kokoro_knows() {
        let known = super::super::KOKORO_SYMBOLS;
        for word in ["pãozinho", "exemplo", "quilômetros", "guitarra", "extraordinário", "piauí", "ãe", "xyz"] {
            for c in ipa(word).chars() {
                assert!(known.contains(c), "{word}: {c:?} is not in Kokoro's vocabulary");
            }
        }
    }
}
