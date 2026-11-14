use std::collections::HashSet;

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "onto", "are", "was", "were", "been", "being", "have",
    "has", "had", "not", "but", "you", "your", "our", "can", "could", "should", "would", "will", "shall", "may",
    "might", "must", "does", "did", "doing", "done", "then", "than", "when", "what", "which", "who", "whom", "why",
    "how", "where", "there", "here", "its", "it's", "all", "any", "some", "each", "every", "also", "just", "only",
    "very", "more", "most", "less", "such", "same", "other", "about", "above", "below", "after", "before", "again",
    "once", "out", "over", "under", "too", "now", "use", "using", "used", "make", "made", "please", "want", "need",
    "like", "get", "got", "let", "fix", "add", "change", "update", "remove", "code", "file", "files", "function",
    "method", "thing", "things", "work", "works", "working", "way", "see", "look", "check", "sure", "think", "know",
    "them", "they", "their", "these", "those", "his", "her", "him", "she", "one", "two", "new", "via", "per", "src",
    "crates", "crate", "lib", "mod", "rs", "tests", "test", "index", "main",
];

pub(crate) fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

pub(crate) fn split_identifier(identifier: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    let chars: Vec<char> = identifier.chars().collect();
    for (index, &ch) in chars.iter().enumerate() {
        if !ch.is_alphanumeric() {
            push(&mut parts, &mut current);
            previous = None;
            continue;
        }
        let next_is_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
        let hump = match previous {
            Some(prev) if ch.is_uppercase() => {
                prev.is_lowercase() || prev.is_numeric() || (prev.is_uppercase() && next_is_lower)
            }
            _ => false,
        };
        if hump {
            push(&mut parts, &mut current);
        }
        current.extend(ch.to_lowercase());
        previous = Some(ch);
    }
    push(&mut parts, &mut current);
    parts
}

fn push(parts: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        parts.push(std::mem::take(current));
    }
}

pub(crate) fn significant(parts: impl IntoIterator<Item = String>) -> HashSet<String> {
    parts.into_iter().filter(|word| word.chars().count() >= 3 && !is_stop_word(word)).map(|word| stem(&word)).collect()
}

pub(crate) fn stem(word: &str) -> String {
    let base = if let Some(stem) = word.strip_suffix("ies").filter(|stem| stem.len() >= 3) {
        format!("{stem}y")
    } else if let Some(stem) = ["ing", "ed", "es", "s"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix).filter(|stem| stem.len() >= 4 && !stem.ends_with('s')))
    {
        stem.to_owned()
    } else {
        word.to_owned()
    };
    base.strip_suffix('e').filter(|stem| stem.len() >= 3).map_or(base.clone(), str::to_owned)
}

pub(crate) struct PromptTerms {
    pub words: HashSet<String>,
    pub identifiers: HashSet<String>,
    pub plain: HashSet<String>,
    pub paths: Vec<String>,
}

pub(crate) fn prompt_terms(prompt: &str) -> PromptTerms {
    let tokens: Vec<&str> = prompt
        .split(|ch: char| !(ch.is_alphanumeric() || matches!(ch, '_' | ':' | '/' | '.' | '-')))
        .map(|token| token.trim_matches(|ch: char| matches!(ch, ':' | '.' | '-' | '/')))
        .filter(|token| !token.is_empty())
        .collect();
    let mut terms =
        PromptTerms { words: HashSet::new(), identifiers: HashSet::new(), plain: HashSet::new(), paths: Vec::new() };
    for token in tokens {
        if token.contains('/') || has_extension(token) {
            terms.paths.push(token.to_owned());
        }
        for piece in token.split([':', '/', '.', '-']).filter(|piece| !piece.is_empty()) {
            if is_identifier_shaped(piece) {
                terms.identifiers.insert(piece.to_owned());
            } else if piece.chars().count() >= 3 && !is_stop_word(&piece.to_lowercase()) {
                terms.plain.insert(stem(&piece.to_lowercase()));
            }
            terms.words.extend(significant(split_identifier(piece)));
        }
    }
    terms
}

fn has_extension(token: &str) -> bool {
    token
        .rsplit_once('.')
        .is_some_and(|(stem, extension)| !stem.is_empty() && crate::extract::supported(&format!("x.{extension}")))
}

fn is_identifier_shaped(piece: &str) -> bool {
    let inner_upper = piece.chars().skip(1).any(char::is_uppercase) && piece.chars().any(char::is_lowercase);
    let snake = piece.trim_matches('_').contains('_');
    (inner_upper || snake) && piece.chars().count() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_split_on_case_humps_underscores_and_acronyms() {
        assert_eq!(split_identifier("merge_near_duplicates"), vec!["merge", "near", "duplicates"]);
        assert_eq!(split_identifier("drawStatusLine"), vec!["draw", "status", "line"]);
        assert_eq!(split_identifier("HTTPServer2Config"), vec!["http", "server2", "config"]);
        assert_eq!(split_identifier("SCREAMING_CASE"), vec!["screaming", "case"]);
    }

    #[test]
    fn stems_join_plurals_and_verb_forms() {
        assert_eq!(stem("memories"), stem("memory"));
        assert_eq!(stem("merging"), stem("merge"));
        assert_eq!(stem("loaded"), stem("load"));
        assert_eq!(stem("class"), "class");
    }

    #[test]
    fn prompt_terms_separate_identifiers_plain_words_and_paths() {
        let terms = prompt_terms("Why is `merge_near_duplicates` in src/dedup.rs slow for the Memory store?");
        assert!(terms.identifiers.contains("merge_near_duplicates"));
        assert!(terms.plain.contains(&stem("memory")));
        assert!(!terms.plain.contains("the"));
        assert!(!terms.plain.contains("is"));
        assert_eq!(terms.paths, vec!["src/dedup.rs"]);
        assert!(terms.words.contains(&stem("duplicates")));
    }
}
