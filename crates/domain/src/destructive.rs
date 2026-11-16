use crate::permission_rules::{normalized, split_command};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Destroys {
    pub paths: Vec<String>,
    pub untracked: bool,
    pub ignored: bool,
}

impl Destroys {
    pub fn anything(&self) -> bool {
        !self.paths.is_empty() || self.untracked
    }
}

pub fn destroys(command: &str) -> Destroys {
    let (parts, _) = split_command(command);
    let mut found = Destroys::default();
    for part in parts {
        let run = normalized(part);
        let owned = words(&run);
        let words: Vec<&str> = owned.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["rm", rest @ ..] => found.paths.extend(removed(rest)),
            ["git", "clean", rest @ ..] => {
                found.untracked = true;
                found.ignored |= rest.iter().any(|flag| {
                    flag.starts_with('-') && !flag.starts_with("--") && (flag.contains('x') || flag.contains('X'))
                });
            }
            ["git", "reset", rest @ ..] if rest.contains(&"--hard") => found.untracked = true,
            ["git", "checkout" | "restore", rest @ ..] if rest.iter().any(|word| *word == "." || *word == "--") => {
                found.untracked = true
            }
            ["git", "stash", rest @ ..]
                if rest.iter().any(|word| matches!(*word, "-u" | "--include-untracked" | "-a" | "--all")) =>
            {
                found.untracked = true
            }
            _ => {}
        }
    }
    found
}

pub(crate) fn words(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in text.chars() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !word.is_empty() {
                    found.push(std::mem::take(&mut word));
                }
                started = false;
            }
            (None, c) => word.push(c),
        }
    }
    if started || !word.is_empty() {
        found.push(word);
    }
    found
}

fn removed(words: &[&str]) -> Vec<String> {
    let recursive = words.iter().any(|word| {
        *word == "--recursive"
            || (word.starts_with('-') && !word.starts_with("--") && (word.contains('r') || word.contains('R')))
    });
    let mut paths = Vec::new();
    let mut options_end = false;
    for word in words {
        if !options_end && *word == "--" {
            options_end = true;
        } else if options_end || !word.starts_with('-') {
            paths.push((*word).to_owned());
        }
    }
    if recursive || !paths.is_empty() { paths } else { Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removal_names_what_it_removes() {
        assert_eq!(destroys("rm -rf build 'my notes'").paths, ["build", "my notes"]);
        assert_eq!(destroys("cd x && sudo rm -- -odd.txt").paths, ["-odd.txt"]);
        assert_eq!(destroys("rm a.txt; echo done").paths, ["a.txt"]);
        assert!(!destroys("echo rm -rf build").anything());
        assert!(!destroys("ls -la").anything());
    }

    #[test]
    fn git_commands_that_drop_work_outside_the_index_are_recognised() {
        assert!(destroys("git reset --hard origin/main").untracked);
        assert!(!destroys("git reset --soft HEAD~1").anything());
        let clean = destroys("git clean -fdx");
        assert!(clean.untracked && clean.ignored);
        assert!(!destroys("git clean -fd").ignored);
        assert!(destroys("git checkout -- .").untracked);
        assert!(destroys("git restore .").untracked);
        assert!(!destroys("git checkout main").anything());
        assert!(destroys("git stash -u").untracked);
        assert!(!destroys("git status").anything());
    }
}
