//! What the user sees of a file change: a heading with the counts, then
//! numbered lines marked `+` (added), `-` (removed) or ` ` (around them):
//! `   12 + let x = 1;`. Only for the user's eyes ([`Outcome::shown`]).
//!
//! [`Outcome::shown`]: super::Outcome::shown

/// Lines shown at most (the chat folds them until asked); the rest is
/// counted.
const LINES: usize = 500;

/// The change of replacing `old` with `new` where `before` (the file's
/// text up to it) ends.
pub fn edit(heading: &str, before: &str, old: &str, new: &str) -> String {
    let first = before.matches('\n').count() + 1;
    let old: Vec<&str> = old.split('\n').collect();
    let new: Vec<&str> = new.split('\n').collect();
    let same_start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let same_end =
        old[same_start..].iter().rev().zip(new[same_start..].iter().rev()).take_while(|(a, b)| a == b).count();
    let removed = &old[same_start..old.len() - same_end];
    let added = &new[same_start..new.len() - same_end];
    let start = first + same_start;
    let mut lines = Vec::new();
    if same_start > 0 {
        lines.push(line(start - 1, ' ', old[same_start - 1]));
    }
    lines.extend(removed.iter().enumerate().map(|(i, text)| line(start + i, '-', text)));
    lines.extend(added.iter().enumerate().map(|(i, text)| line(start + i, '+', text)));
    if same_end > 0 {
        lines.push(line(start + added.len(), ' ', old[old.len() - same_end]));
    }
    let counts = format!("Added {} lines, removed {}", added.len(), removed.len());
    finish(heading, &counts, lines)
}

/// A new file's first lines, all added.
pub fn created(heading: &str, content: &str) -> String {
    let lines: Vec<String> = content.lines().enumerate().map(|(i, text)| line(i + 1, '+', text)).collect();
    let counts = format!("Wrote {} lines", lines.len());
    finish(heading, &counts, lines)
}

fn line(number: usize, mark: char, text: &str) -> String {
    format!("{number:>5} {mark} {text}")
}

fn finish(heading: &str, counts: &str, mut lines: Vec<String>) -> String {
    let more = lines.len().saturating_sub(LINES);
    lines.truncate(LINES);
    if more > 0 {
        lines.push(format!("      … +{more} lines"));
    }
    format!("{heading} · {counts}\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_shows_only_what_changed_with_its_line_numbers() {
        let shown = edit(
            "Edited f.rs",
            "fn a() {}\nfn main() {\n",
            "    let x = 1;\n    run(x);",
            "    let x = 2;\n    run(x);",
        );
        assert_eq!(
            shown,
            "Edited f.rs · Added 1 lines, removed 1\n    3 -     let x = 1;\n    3 +     let x = 2;\n    4       run(x);"
        );
    }

    #[test]
    fn a_new_file_is_all_added_and_long_ones_are_cut() {
        assert_eq!(created("Created a.txt", "one\ntwo"), "Created a.txt · Wrote 2 lines\n    1 + one\n    2 + two");
        let long = (0..510).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        assert!(created("Created b.txt", &long).ends_with("… +10 lines"));
    }
}
