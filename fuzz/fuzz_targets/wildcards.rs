#![no_main]

use arbitrary::{Result, Unstructured};
use domain::command_policy::{alternatives, could_match};
use domain::permission_rules::{command_matches, path_matches};
use libfuzzer_sys::fuzz_target;

const PATTERN: &[u8] = b"ab.*? ";
const NAME: &[u8] = b"ab.?* ";

fn spelled(u: &mut Unstructured<'_>, from: &[u8], most: usize) -> Result<String> {
    let length = u.int_in_range(0..=most)?;
    (0..length).map(|_| u.choose(from).map(|&byte| char::from(byte))).collect()
}

fn glob(pattern: &[char], text: &[char], question: bool) -> bool {
    let mut meets = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    meets[pattern.len()][text.len()] = true;
    for p in (0..pattern.len()).rev() {
        for t in (0..=text.len()).rev() {
            meets[p][t] = match pattern[p] {
                '*' => meets[p + 1][t] || (t < text.len() && meets[p][t + 1]),
                '?' if question => t < text.len() && meets[p + 1][t + 1],
                c => t < text.len() && text[t] == c && meets[p + 1][t + 1],
            };
        }
    }
    meets[0][0]
}

fn globbed(pattern: &str, text: &str, question: bool) -> bool {
    let (pattern, text): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    glob(&pattern, &text, question)
}

fn commands(u: &mut Unstructured<'_>) -> Result<()> {
    let mut specifier = spelled(u, PATTERN, 10)?;
    if !specifier.contains('*') {
        specifier.push('*');
    }
    if specifier.ends_with(":*") {
        return Ok(());
    }
    let command = spelled(u, NAME, 12)?;
    assert_eq!(
        command_matches(&specifier, &command),
        globbed(&specifier, &command, false),
        "command {specifier:?} against {command:?}"
    );
    Ok(())
}

fn paths(u: &mut Unstructured<'_>) -> Result<()> {
    let specifier = spelled(u, PATTERN, 8)?;
    if specifier.is_empty() || specifier.trim().is_empty() || specifier == "**" {
        return Ok(());
    }
    let count = u.int_in_range(0..=4)?;
    let names = (0..count).map(|_| spelled(u, NAME, 6)).collect::<Result<Vec<_>>>()?;
    let path = format!("/w/{}", names.join("/"));
    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
    let expected = segments.iter().skip(1).any(|segment| globbed(&specifier, segment, true));
    assert_eq!(path_matches(&specifier, &path, "/w", None, false), expected, "path {specifier:?} against {path:?}");
    Ok(())
}

fn meetings(u: &mut Unstructured<'_>) -> Result<()> {
    let word = spelled(u, b"ab.*?[]{},", 120)?;
    assert!(could_match(&word, &word), "{word:?} could not share a file with itself");
    let other = spelled(u, b"ab.*?[]", 20)?;
    let _ = could_match(&word, &other);
    let found = alternatives(&word);
    assert!(!found.is_empty());
    if !word.contains('{') {
        assert_eq!(found, [word]);
    }
    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let _ = commands(&mut u);
    let _ = paths(&mut u);
    let _ = meetings(&mut u);
});
