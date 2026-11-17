const MOST_ALTERNATIVES: usize = 64;
const LONGEST_PATTERN: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Run,
    One,
    OneOf,
    Just(char),
}

pub fn has_wildcards(word: &str) -> bool {
    word.contains(['*', '?', '['])
}

pub fn could_match(word: &str, pattern: &str) -> bool {
    let (word, pattern) = (pieces(word), pieces(pattern));
    let hidden = pattern.first() == Some(&Piece::Just('.'));
    let skips_hidden = !matches!(word.first(), Some(Piece::Just(_)) | None);
    if hidden && skips_hidden {
        return false;
    }
    word.len() > LONGEST_PATTERN || meet(&word, &pattern)
}

pub fn alternatives(word: &str) -> Vec<String> {
    let mut found = vec![word.to_owned()];
    while let Some(at) = found.iter().position(|word| group(word).is_some()) {
        if found.len() > MOST_ALTERNATIVES {
            break;
        }
        let word = found.swap_remove(at);
        if let Some((open, close)) = group(&word) {
            let (head, tail) = (&word[..open], &word[close + 1..]);
            found.extend(word[open + 1..close].split(',').map(|choice| format!("{head}{choice}{tail}")));
        }
    }
    found
}

fn group(word: &str) -> Option<(usize, usize)> {
    word.match_indices('}').find_map(|(close, _)| {
        let open = word[..close].rfind('{')?;
        word[open..close].contains(',').then_some((open, close))
    })
}

fn pieces(text: &str) -> Vec<Piece> {
    let mut found = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => found.push(Piece::Run),
            '?' => found.push(Piece::One),
            '[' => {
                let closed = chars.clone().skip(1).any(|c| c == ']');
                if closed {
                    chars.next();
                    for inner in chars.by_ref() {
                        if inner == ']' {
                            break;
                        }
                    }
                    found.push(Piece::OneOf);
                } else {
                    found.push(Piece::Just(c));
                }
            }
            other => found.push(Piece::Just(other)),
        }
    }
    found
}

fn meet(a: &[Piece], b: &[Piece]) -> bool {
    let width = b.len() + 1;
    let mut meets = vec![false; (a.len() + 1) * width];
    for i in (0..=a.len()).rev() {
        for j in (0..=b.len()).rev() {
            let at = |i: usize, j: usize| meets[i * width + j];
            let met = match (a.get(i), b.get(j)) {
                (None, None) => true,
                (Some(Piece::Run), _) => at(i + 1, j) || (j < b.len() && at(i, j + 1)),
                (_, Some(Piece::Run)) => at(i, j + 1) || (i < a.len() && at(i + 1, j)),
                (None, _) | (_, None) => false,
                (Some(x), Some(y)) => alike(x, y) && at(i + 1, j + 1),
            };
            meets[i * width + j] = met;
        }
    }
    meets[0]
}

fn alike(a: &Piece, b: &Piece) -> bool {
    match (a, b) {
        (Piece::Just(a), Piece::Just(b)) => a == b,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wildcard_meets_a_name_or_another_wildcard_it_could_share_a_file_with() {
        for (word, pattern) in [
            (".env*", ".env"),
            (".en?", ".env"),
            ("id_*", "id_rsa"),
            (".e[n]v", ".env"),
            ("serv*", "*.pem"),
            ("*", "*.key"),
            (".env.pro*", ".env.*"),
        ] {
            assert!(could_match(word, pattern), "{word} {pattern}");
        }
        for (word, pattern) in [("*.txt", "*.pem"), ("*.rs", ".env"), ("id_?", "id_rsa"), ("notes*", ".env.*")] {
            assert!(!could_match(word, pattern), "{word} {pattern}");
        }
    }

    fn met_by_trying_every_way(a: &[Piece], b: &[Piece]) -> bool {
        match (a.split_first(), b.split_first()) {
            (None, None) => true,
            (Some((Piece::Run, rest)), _) => {
                met_by_trying_every_way(rest, b) || (!b.is_empty() && met_by_trying_every_way(a, &b[1..]))
            }
            (_, Some((Piece::Run, rest))) => {
                met_by_trying_every_way(a, rest) || (!a.is_empty() && met_by_trying_every_way(&a[1..], b))
            }
            (None, _) | (_, None) => false,
            (Some((x, a)), Some((y, b))) => alike(x, y) && met_by_trying_every_way(a, b),
        }
    }

    #[test]
    fn two_wildcards_meet_exactly_when_trying_every_way_says_they_do() {
        const PIECES: [&str; 7] = ["*", "?", "[ab]", "a", "b", ".", "e"];
        let mut random = crate::shell::sample::Random(0x0bad_5eed_c0ff_ee01);
        for _ in 0..crate::shell::sample::cases().max(2_000) {
            let mut word = || (0..random.below(7)).map(|_| PIECES[random.below(PIECES.len())]).collect::<String>();
            let (a, b) = (word(), word());
            let (x, y) = (pieces(&a), pieces(&b));
            assert_eq!(meet(&x, &y), met_by_trying_every_way(&x, &y), "{a:?} {b:?}");
        }
    }

    #[test]
    fn many_stars_are_matched_at_once_not_tried_one_way_after_another() {
        let started = std::time::Instant::now();
        assert!(!meet(&pieces(&format!("{}Z", "*".repeat(200))), &pieces("id_*.pub.pem.key.X")));
        assert!(!could_match(&format!("{}Z", "*".repeat(90)), "*.pem"));
        assert!(could_match(&format!("{}m", "*".repeat(90)), "*.pem"));
        assert!(started.elapsed() < std::time::Duration::from_millis(50), "{:?}", started.elapsed());
    }

    #[test]
    fn a_class_is_taken_to_match_any_character_rather_than_guessed_at() {
        assert!(could_match(".e[a-m]v", ".env"));
    }

    #[test]
    fn braces_are_spelled_out_into_each_word_they_stand_for() {
        let mut found = alternatives(".{env,x{a,b}}");
        found.sort();
        found.dedup();
        assert_eq!(found, [".env", ".xa", ".xb"]);
        assert_eq!(alternatives("a}{b,c}"), ["a}b", "a}c"]);
        assert_eq!(alternatives("{}"), ["{}"]);
        assert_eq!(alternatives("plain"), ["plain"]);
    }
}
