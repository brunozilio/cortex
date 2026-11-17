use crate::permission_rules::without_harmless_redirects;

pub fn only_describes(part: &str) -> bool {
    let part = without_harmless_redirects(part);
    let words: Vec<&str> = part.split_whitespace().collect();
    if !words.iter().all(|word| plain(word)) {
        return false;
    }
    match words.as_slice() {
        ["command", "-v" | "-V", names @ ..] => !names.is_empty(),
        [program, "--version" | "--help"] => *program == brand::COMMAND || super::reading::is_reader(program),
        [program, "mcp", "list"] => *program == brand::COMMAND,
        [program, subcommands @ .., "--help"] => {
            *program == brand::COMMAND
                && subcommands.len() <= 2
                && subcommands.iter().all(|word| word.chars().all(|c| c.is_ascii_lowercase()))
        }
        _ => false,
    }
}

fn plain(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:@+".contains(c))
}
