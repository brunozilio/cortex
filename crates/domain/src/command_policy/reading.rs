const READERS: [&str; 30] = [
    "ls", "pwd", "cat", "head", "tail", "wc", "grep", "egrep", "fgrep", "rg", "tree", "file", "stat", "which", "du",
    "df", "diff", "uniq", "cut", "basename", "dirname", "realpath", "nl", "tac", "jq", "type", "true", "false", "date",
    "whoami",
];

const GIT_READERS: [&str; 10] =
    ["status", "log", "diff", "show", "rev-parse", "ls-files", "blame", "grep", "describe", "shortlog"];

pub fn only_reads(words: &[&str]) -> bool {
    match words {
        [] => false,
        ["git", "branch", rest @ ..] => rest.iter().all(|word| word.starts_with('-') && !branch_writes(word)),
        ["git", "remote"] | ["git", "remote", "-v"] => true,
        ["git", "tag"] | ["git", "tag", "-l" | "--list", ..] => true,
        ["git", sub, rest @ ..] => {
            GIT_READERS.contains(sub) && inside(rest) && !rest.iter().any(|word| word.starts_with("--output"))
        }
        ["find", rest @ ..] => inside(rest) && !rest.iter().any(|word| finds_and_acts(word)),
        ["sort", rest @ ..] => inside(rest) && !rest.iter().any(|word| word.starts_with("-o") || *word == "--output"),
        [program, rest @ ..] => READERS.contains(program) && inside(rest),
    }
}

fn branch_writes(flag: &str) -> bool {
    matches!(
        flag,
        "-d" | "-D" | "-m" | "-M" | "-c" | "-C" | "--delete" | "--move" | "--copy" | "-u" | "--set-upstream-to"
    ) || flag.starts_with("--set-upstream")
        || flag.starts_with("--edit-description")
}

fn finds_and_acts(word: &str) -> bool {
    matches!(word, "-exec" | "-execdir" | "-ok" | "-okdir" | "-delete" | "-fprint" | "-fprint0" | "-fprintf" | "-fls")
}

fn inside(words: &[&str]) -> bool {
    words.iter().all(|word| {
        let path = word.split_once('=').map_or(*word, |(_, value)| value);
        !path.starts_with('/') && !path.starts_with('~') && !path.split('/').any(|segment| segment == "..")
    })
}
