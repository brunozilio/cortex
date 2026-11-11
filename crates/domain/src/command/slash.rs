pub fn slash_name(text: &str) -> Option<&str> {
    let text = text.trim().strip_prefix('/')?;
    let name = text.split_whitespace().next()?;
    let command_like = name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '?'));
    command_like.then_some(name)
}

pub fn closest<'a>(name: &str, known: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let name = name.to_ascii_lowercase();
    let allowed = (name.chars().count() / 3).clamp(1, 3);
    known
        .into_iter()
        .map(|candidate| (distance(&name, &candidate.to_ascii_lowercase()), candidate))
        .filter(|(distance, _)| *distance <= allowed)
        .min_by_key(|(distance, candidate)| (*distance, name.len().abs_diff(candidate.len())))
        .map(|(_, candidate)| candidate)
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut table = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in table.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in table[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (table[i - 1][j] + 1).min(table[i][j - 1] + 1).min(table[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(table[i - 2][j - 2] + 1);
            }
            table[i][j] = best;
        }
    }
    table[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_command_shaped_first_word_is_a_slash_name() {
        assert_eq!(slash_name("/mode sideways"), Some("mode"));
        assert_eq!(slash_name("  /plugin:review now"), Some("plugin:review"));
        assert_eq!(slash_name("/mcp__jira__search bugs"), Some("mcp__jira__search"));
        assert_eq!(slash_name("/Users/me/notes.txt what is this?"), None, "a path is a prompt");
        assert_eq!(slash_name("/"), None);
        assert_eq!(slash_name("fix /tmp"), None);
    }

    #[test]
    fn a_typo_finds_the_nearest_name_and_nonsense_finds_none() {
        let known = ["model", "mode", "memory", "compact", "resume"];
        assert_eq!(closest("modle", known), Some("model"));
        assert_eq!(closest("Compcat", known), Some("compact"));
        assert_eq!(closest("resum", known), Some("resume"));
        assert_eq!(closest("zzzzzz", known), None);
    }
}
