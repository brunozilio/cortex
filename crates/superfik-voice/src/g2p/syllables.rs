//! Where a Portuguese word's vowels are and which one carries the stress,
//! from its spelling alone. Pure.

pub(super) fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'á' | 'é' | 'í' | 'ó' | 'ú' | 'â' | 'ê' | 'ô' | 'ã' | 'õ' | 'à' | 'ü'
    )
}

fn is_marked(c: char) -> bool {
    matches!(c, 'á' | 'é' | 'í' | 'ó' | 'ú' | 'â' | 'ê' | 'ô')
}

pub(super) fn is_silent_u(word: &[char], i: usize) -> bool {
    word[i] == 'u'
        && i > 0
        && matches!(word[i - 1], 'q' | 'g')
        && word.get(i + 1).is_some_and(|c| matches!(c, 'e' | 'i' | 'é' | 'ê' | 'í'))
}

pub(super) fn is_glide_u(word: &[char], i: usize) -> bool {
    word.get(i) == Some(&'u')
        && i > 0
        && matches!(word[i - 1], 'q' | 'g')
        && word.get(i + 1).is_some_and(|c| matches!(c, 'a' | 'o' | 'á' | 'ó' | 'â' | 'ã' | 'ô'))
}

pub(super) fn nuclei(word: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < word.len() {
        if !is_vowel(word[i]) || is_silent_u(word, i) || is_glide_u(word, i) {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        if i < word.len() && glides(word, start, i) {
            i += 1;
        }
        out.push((start, i));
    }
    out
}

fn glides(word: &[char], before: usize, at: usize) -> bool {
    let (first, next) = (word[before], word[at]);
    if matches!((first, next), ('ã', 'o' | 'e') | ('õ', 'e')) {
        return true;
    }
    if !matches!(next, 'i' | 'u') || matches!(first, 'i' | 'í' | 'u' | 'ú') && next == first {
        return false;
    }
    let rest = &word[at + 1..];
    let consonant = |c: &char| !is_vowel(*c);
    match rest {
        ['n', 'h', ..] => false,
        [c, ..] if matches!(c, 'm' | 'n' | 'r' | 'l' | 'z') && rest.get(1).is_none_or(consonant) => false,
        _ => true,
    }
}

const CLITICS: &[&str] = &[
    "a", "o", "as", "os", "e", "de", "do", "da", "dos", "das", "em", "no", "na", "nos", "nas", "que", "se", "me", "te",
    "lhe", "lhes", "por", "com", "sem", "mas", "ou", "ao", "aos", "à", "às", "pra", "pro", "num", "numa",
];

pub(super) fn stressed(word: &[char], nuclei: &[(usize, usize)]) -> Option<usize> {
    if nuclei.is_empty() {
        return None;
    }
    let text: String = word.iter().collect();
    if nuclei.len() == 1 {
        return (!CLITICS.contains(&text.as_str())).then_some(0);
    }
    let holding = |pred: fn(char) -> bool| nuclei.iter().position(|&(s, e)| word[s..e].iter().any(|&c| pred(c)));
    if let Some(marked) = holding(is_marked) {
        return Some(marked);
    }
    if let Some(tilde) = holding(|c| matches!(c, 'ã' | 'õ')) {
        return Some(tilde);
    }
    let last = nuclei.len() - 1;
    let paroxytone = ["a", "e", "o", "as", "es", "os", "am", "em", "ens"].iter().any(|ending| {
        text.ends_with(ending) && {
            let tail = text.chars().count() - ending.chars().count();
            nuclei[last].0 >= tail
        }
    });
    Some(if paroxytone { last - 1 } else { last })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(word: &str) -> (Vec<String>, Option<usize>) {
        let chars: Vec<char> = word.chars().collect();
        let found = nuclei(&chars);
        let parts = found.iter().map(|&(s, e)| chars[s..e].iter().collect()).collect();
        (parts, stressed(&chars, &found))
    }

    #[test]
    fn diphthongs_are_one_nucleus_and_hiatus_two() {
        assert_eq!(split("saudade").0, ["au", "a", "e"]);
        assert_eq!(split("muito").0, ["ui", "o"]);
        assert_eq!(split("dia").0, ["i", "a"]);
        assert_eq!(split("rainha").0, ["a", "i", "a"]);
        assert_eq!(split("ainda").0, ["a", "i", "a"]);
        assert_eq!(split("coração").0, ["o", "a", "ão"]);
        assert_eq!(split("aquele").0, ["a", "e", "e"], "the u of que is silent");
    }

    #[test]
    fn stress_follows_accents_then_the_ending() {
        assert_eq!(split("obrigado").1, Some(2), "a paroxytone");
        assert_eq!(split("computador").1, Some(3), "ending in r: the last");
        assert_eq!(split("café").1, Some(1));
        assert_eq!(split("lâmpada").1, Some(0));
        assert_eq!(split("coração").1, Some(2));
        assert_eq!(split("órfãs").1, Some(0), "the acute wins over the tilde");
        assert_eq!(split("aquele").1, Some(1));
        assert_eq!(split("juiz").1, Some(1));
        assert_eq!(split("fazem").1, Some(0));
        assert_eq!(split("Brasil".to_lowercase().as_str()).1, Some(1));
    }

    #[test]
    fn clitics_lean_and_other_monosyllables_stand() {
        assert_eq!(split("de").1, None);
        assert_eq!(split("que").1, None);
        assert_eq!(split("voz").1, Some(0));
        assert_eq!(split("bem").1, Some(0));
    }
}
