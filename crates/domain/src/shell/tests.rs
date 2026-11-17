use super::*;

fn commands(text: &str) -> Vec<String> {
    let parsed = ShellCommand::parse(text);
    assert!(!parsed.opaque(), "{text} should parse");
    parsed.commands().into_iter().map(str::to_owned).collect()
}

fn opaque(text: &str) -> bool {
    ShellCommand::parse(text).opaque()
}

#[test]
fn lists_and_pipelines_split_into_their_simple_commands() {
    assert_eq!(commands("ls -la"), ["ls -la"]);
    assert_eq!(commands("a && b || c; d & e | f |& g"), ["a", "b", "c", "d", "e", "f", "g"]);
    assert_eq!(commands("cargo build 2>&1 | tail -20"), ["cargo build 2>&1", "tail -20"]);
    assert_eq!(commands("echo 'a && b; c' \"d | e\""), ["echo 'a && b; c' \"d | e\""]);
    assert_eq!(commands("a\nb\n\n  c"), ["a", "b", "c"]);
    assert_eq!(commands("X=1 Y=2 make &> out.txt"), ["X=1 Y=2 make &> out.txt"]);
    assert_eq!(commands("ls # a comment; rm x"), ["ls"]);
    assert_eq!(commands("ls \\\n  -la"), ["ls \\\n  -la"]);
    assert!(ShellCommand::parse("ls -la | wc -l").flat());
}

#[test]
fn compound_commands_are_walked_into_and_are_not_flat() {
    for (text, expected) in [
        ("( ls; rm x )", vec!["ls", "rm x"]),
        ("{ rm x; }", vec!["rm x"]),
        ("! rm x", vec!["rm x"]),
        ("if true; then rm -rf x; fi", vec!["true", "rm -rf x"]),
        ("if a; then b; elif c; then d; else e; fi", vec!["a", "b", "c", "d", "e"]),
        ("for f in *.rs; do rm $f; done", vec!["rm $f"]),
        ("for f\ndo echo $f\ndone", vec!["echo $f"]),
        ("while read l; do echo $l; done < in.txt", vec!["read l", "echo $l"]),
        ("until rm x; do ls; done", vec!["rm x", "ls"]),
        ("case $x in a|b) rm x ;; (*) ls;; esac", vec!["rm x", "ls"]),
        ("f() { rm x; }; f", vec!["rm x", "f"]),
        ("function f { rm x; }", vec!["rm x"]),
        ("function f() ( rm x )", vec!["rm x"]),
        ("[[ -f a && $x < b ]] && rm x", vec!["[[ -f a && $x < b ]]", "rm x"]),
    ] {
        let parsed = ShellCommand::parse(text);
        assert!(!parsed.opaque(), "{text}");
        assert!(!parsed.flat(), "{text}");
        assert_eq!(parsed.commands(), expected, "{text}");
    }
}

#[test]
fn substitutions_are_parsed_as_commands_of_their_own() {
    assert_eq!(commands("echo $(rm -rf x)"), ["echo $(rm -rf x)", "rm -rf x"]);
    assert_eq!(commands("echo \"$(git status) `id`\""), ["echo \"$(git status) `id`\"", "git status", "id"]);
    assert_eq!(commands("diff <(ls a) >(cat)"), ["diff <(ls a) >(cat)", "ls a", "cat"]);
    assert_eq!(commands("echo $((1 + 2)) ${x:-y} $'a\\'b' $\"c\""), ["echo $((1 + 2)) ${x:-y} $'a\\'b' $\"c\""]);
    assert!(!ShellCommand::parse("echo $(date)").flat());
}

#[test]
fn heredocs_are_read_as_data_after_their_line() {
    assert_eq!(commands("cat <<EOF\nrm -rf x\nEOF\nls"), ["cat <<EOF", "ls"]);
    assert_eq!(commands("cat <<-'EOF' | wc -l\n\t$(rm x)\n\tEOF"), ["cat <<-'EOF'", "wc -l"]);
    assert_eq!(commands("cat <<< 'here string'"), ["cat <<< 'here string'"]);
    assert!(opaque("cat <<EOF\n$(rm x)\nEOF"), "an unquoted heredoc runs what it substitutes");
    assert!(opaque("cat <<EOF\nnever closed"));
}

#[test]
fn a_command_run_by_find_is_a_command_of_its_own() {
    assert_eq!(commands("find . -name '*.tmp' -exec rm {} \\;"), ["find . -name '*.tmp' -exec rm {} \\;", "rm {}"]);
    assert_eq!(commands("find . -execdir sh -c 'x' + -print"), ["find . -execdir sh -c 'x' + -print", "sh -c 'x'"]);
    assert_eq!(commands("sudo find . -ok rm {} ';'"), ["sudo find . -ok rm {} ';'", "rm {}"]);
}

#[test]
fn what_cannot_be_parsed_with_confidence_is_opaque() {
    for text in [
        "echo 'unclosed",
        "echo \"unclosed",
        "echo $(unclosed",
        "echo `unclosed",
        "( ls",
        "ls )",
        "if true; then ls",
        "for ((i = 0; i < 3; i++)); do ls; done",
        "(( x++ ))",
        "coproc ls",
        "echo ${x:-$(rm x)}",
        "echo $(( $(id) ))",
        "echo `echo \\`id\\``",
        "fi",
        "ls >",
        "ls &&",
        "echo (x)",
    ] {
        assert!(opaque(text), "{text}");
    }
}

#[test]
fn an_opaque_line_is_still_split_roughly_so_rules_can_see_its_words() {
    let parsed = ShellCommand::parse("ls; rm 'x");
    assert!(parsed.opaque() && !parsed.flat());
    assert_eq!(parsed.commands(), ["ls", "rm 'x"]);
}

#[test]
fn nesting_deeper_than_the_parser_follows_is_opaque_not_a_crash() {
    let deep = format!("{}ls{}", "$(".repeat(200), ")".repeat(200));
    assert!(opaque(&deep));
    let subshells = format!("{}ls{}", "( ".repeat(200), " )".repeat(200));
    assert!(opaque(&subshells));
}

use super::sample::{Random, cases};

#[test]
fn the_parser_never_panics_on_any_text() {
    const PIECES: [&str; 40] = [
        " ", "\t", "\n", ";", ";;", "&", "&&", "|", "||", "(", ")", "{", "}", "<", ">", "<<", "<<-", "'", "\"", "`",
        "$", "$(", "${", "$((", "\\", "#", "!", "[[", "]]", "if", "then", "fi", "for", "in", "do", "done", "case",
        "esac", "é", "EOF",
    ];
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    for _ in 0..cases() {
        let length = random.below(24);
        let text: String =
            (0..length).map(|_| if random.below(3) == 0 { "ls" } else { PIECES[random.below(PIECES.len())] }).collect();
        let parsed = ShellCommand::parse(&text);
        let _ = parsed.commands();
        let _ = simple_commands(&text);
    }
}
