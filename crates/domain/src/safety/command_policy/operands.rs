use std::path::{Component, Path, PathBuf};

use super::readers::Reader;
use super::reading::{reader_of, with_home};
use super::{reads_only, without_harmless_redirects, words};
use crate::shell::{Redirection, ShellCommand, SimpleCommand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operand {
    pub path: PathBuf,
    pub tree: bool,
}

const NAMES_NOT_FILES: [&[&str]; 16] = [
    &["pwd"],
    &["which"],
    &["type"],
    &["basename"],
    &["dirname"],
    &["realpath"],
    &["date"],
    &["true"],
    &["false"],
    &["whoami"],
    &["tr"],
    &["git", "status"],
    &["git", "rev-parse"],
    &["git", "describe"],
    &["git", "branch"],
    &["git", "tag"],
];

const PATTERN_FIRST: [&[&str]; 5] = [&["grep"], &["egrep"], &["fgrep"], &["rg"], &["git", "grep"]];

const ALWAYS_TREES: [&[&str]; 5] = [&["rg"], &["find"], &["du"], &["tree"], &["git", "grep"]];

const RECURSIVE_WHEN_ASKED: [&[&str]; 5] = [&["grep"], &["egrep"], &["fgrep"], &["ls"], &["diff"]];

const REVISIONS_AND_PATHS: [&[&str]; 4] = [&["git", "diff"], &["git", "log"], &["git", "show"], &["git", "blame"]];

fn redirected_reads(command: &ShellCommand, cwd: &Path, home: Option<&str>) -> Vec<Operand> {
    let targets = command
        .commands()
        .into_iter()
        .filter_map(SimpleCommand::of)
        .flat_map(|simple| simple.redirections)
        .filter(Redirection::reads_a_file)
        .map(|redirection| redirection.target.chars().filter(|c| !matches!(c, '\'' | '"' | '\\')).collect::<String>());
    let homed = |target: String| match (target.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => format!("{}/{rest}", home.trim_end_matches('/')),
        _ => target,
    };
    targets
        .map(homed)
        .filter(|target| !target.is_empty() && !target.contains(['$', '`', '*', '?', '[']))
        .map(|target| Operand { path: joined(cwd, &target), tree: false })
        .collect()
}

pub fn paths_read(command: &ShellCommand, cwd: &str, home: Option<&str>) -> Vec<Operand> {
    let mut found = redirected_reads(command, Path::new(cwd), home);
    if !command.flat() {
        return found;
    }
    for part in command.commands() {
        let part = with_home(part, home);
        if !reads_only(&part, &|_| true) {
            continue;
        }
        let words = words(&without_harmless_redirects(&part));
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let Some(reader) = reader_of(&words) else { continue };
        let program = reader.program;
        if NAMES_NOT_FILES.contains(&program) || !reader.operands {
            continue;
        }
        found.extend(operands(reader, &words[program.len()..], Path::new(cwd)));
    }
    found
}

fn operands(reader: &Reader, arguments: &[&str], cwd: &Path) -> Vec<Operand> {
    let program = reader.program;
    let flags = flags(arguments);
    let tree = ALWAYS_TREES.contains(&program) || (RECURSIVE_WHEN_ASKED.contains(&program) && recursive(&flags));
    let mut named = names(reader, arguments);
    if PATTERN_FIRST.contains(&program) && !names_its_pattern(&flags) && !named.is_empty() {
        named.remove(0);
    }
    if REVISIONS_AND_PATHS.contains(&program) {
        named = named.into_iter().map(|word| word.split_once(':').map_or(word, |(_, path)| path)).collect();
    }
    if named.is_empty() && tree {
        named.push(".");
    }
    named.into_iter().filter(|word| !word.is_empty()).map(|word| Operand { path: joined(cwd, word), tree }).collect()
}

fn flags<'a>(arguments: &[&'a str]) -> Vec<&'a str> {
    arguments.iter().take_while(|word| **word != "--").copied().filter(|word| is_flag(word)).collect()
}

fn is_flag(word: &str) -> bool {
    word.starts_with('-') && word != "-"
}

fn names<'a>(reader: &Reader, arguments: &[&'a str]) -> Vec<&'a str> {
    let mut named = Vec::new();
    let mut options_ended = false;
    let mut expression = false;
    let mut value_next = false;
    for word in arguments {
        if std::mem::take(&mut value_next) {
            continue;
        }
        if !options_ended && *word == "--" {
            options_ended = true;
            continue;
        }
        if reader.program == ["find"] && (word.starts_with('-') || matches!(*word, "(" | "!" | ")")) {
            expression = true;
        }
        if expression || *word == "-" {
            continue;
        }
        if !options_ended && is_flag(word) {
            value_next = takes_a_value(reader, word);
            continue;
        }
        named.push(*word);
    }
    named
}

fn takes_a_value(reader: &Reader, flag: &str) -> bool {
    let Some(last) = flag.chars().last().filter(|_| !flag.starts_with("--")) else { return false };
    (flag.len() == 2 || reader.clustered) && reader.flags.contains(&format!("-{last}=").as_str())
}

fn recursive(flags: &[&str]) -> bool {
    flags.iter().any(|flag| {
        *flag == "--recursive"
            || flag.starts_with("--dereference-recursive")
            || (!flag.starts_with("--") && flag.contains(['r', 'R']))
    })
}

fn names_its_pattern(flags: &[&str]) -> bool {
    flags.iter().any(|flag| {
        let name = flag.split_once('=').map_or(*flag, |(name, _)| name);
        matches!(name, "--files" | "--regexp" | "--file") || (!flag.starts_with("--") && flag.contains(['e', 'f']))
    })
}

fn joined(cwd: &Path, word: &str) -> PathBuf {
    let path = if word.starts_with('/') { PathBuf::from(word) } else { cwd.join(word) };
    path.components().filter(|component| *component != Component::CurDir).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(command: &str) -> Vec<Operand> {
        paths_read(&ShellCommand::parse(command), "/work", Some("/home/ana"))
    }

    fn file(path: &str) -> Operand {
        Operand { path: PathBuf::from(path), tree: false }
    }

    fn tree(path: &str) -> Operand {
        Operand { path: PathBuf::from(path), tree: true }
    }

    #[test]
    fn every_file_a_read_only_command_names_is_read_from_where_it_runs() {
        assert_eq!(read("cat ./a b/c /etc/hosts"), [file("/work/a"), file("/work/b/c"), file("/etc/hosts")]);
        assert_eq!(read("head -n 5 a | wc -l"), [file("/work/a")]);
        assert_eq!(read("cat ~/notes"), [file("/home/ana/notes")]);
        assert_eq!(read("cat -- -odd"), [file("/work/-odd")]);
        assert_eq!(read("cat"), []);
    }

    #[test]
    fn a_search_reads_a_tree_and_its_pattern_is_not_a_path() {
        assert_eq!(read("grep token a"), [file("/work/a")]);
        assert_eq!(read("grep -e token a"), [file("/work/a")]);
        assert_eq!(read("grep -rn token src"), [tree("/work/src")]);
        assert_eq!(read("grep -r token"), [tree("/work")]);
        assert_eq!(read("rg token"), [tree("/work")]);
        assert_eq!(read("rg -n . secrets"), [tree("/work/secrets")]);
        assert_eq!(read("git grep token"), [tree("/work")]);
        assert_eq!(read("find src -name '*.rs'"), [tree("/work/src")]);
        assert_eq!(read("find -name x"), [tree("/work")]);
        assert_eq!(read("ls -R docs"), [tree("/work/docs")]);
        assert_eq!(read("ls -la docs"), [file("/work/docs")]);
    }

    #[test]
    fn names_that_are_not_files_and_commands_that_are_not_read_only_give_nothing() {
        assert_eq!(read("git status"), []);
        assert_eq!(read("which cargo"), []);
        assert_eq!(read("echo secrets/key"), []);
        assert_eq!(read("tr a-z A-Z"), []);
        assert_eq!(read("rm secrets/key"), []);
        assert_eq!(read("cat $(echo secrets/key)"), []);
    }

    #[test]
    fn a_file_an_input_redirection_names_is_read_whatever_the_command() {
        assert_eq!(read("<secrets/k cat"), [file("/work/secrets/k")]);
        assert_eq!(read("cat<secrets/k"), [file("/work/secrets/k")]);
        assert_eq!(read("rm x < '/etc/hosts'"), [file("/etc/hosts")]);
        assert_eq!(read("sort <~/notes"), [file("/home/ana/notes")]);
        assert_eq!(read("if true; then wc -l <a; fi"), [file("/work/a")]);
        assert_eq!(read("cat <<EOF\nsecrets/k\nEOF"), []);
        assert_eq!(read("cat <<< secrets/k"), []);
        assert_eq!(read("cat <&3"), []);
        assert_eq!(read("cat <$f"), []);
        assert!(read("ls >out 2>err").is_empty(), "a file written to is not read");
    }

    #[test]
    fn a_git_object_names_the_file_after_its_colon() {
        assert_eq!(read("git show HEAD:secrets/key"), [file("/work/secrets/key")]);
        assert_eq!(read("git diff -- secrets/key"), [file("/work/secrets/key")]);
    }
}
