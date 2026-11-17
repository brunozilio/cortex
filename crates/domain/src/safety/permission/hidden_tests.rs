use std::path::PathBuf;

use super::*;
use crate::PermissionMode;
use crate::permission_rules::{PermissionRules, RuleKind};
use crate::shell::sample::{Random, cases};

const MODES: [PermissionMode; 4] =
    [PermissionMode::Bypass, PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Plan];

fn judged(rules: &PermissionRules, mode: PermissionMode, line: &str) -> Verdict {
    let remembered = Remembered::default();
    let bounds = Bounds::of(vec![PathBuf::from("/work/rocket")], Vec::new());
    let effects = [Effect::exec(line, false)];
    let cx = PolicyContext {
        mode,
        ceiling: None,
        rules,
        remembered: &remembered,
        bounds: &bounds,
        cwd: "/work/rocket",
        home: Some("/home/ana"),
        sandboxed: true,
        sandbox_missing: None,
        no_prompts: false,
        surroundings: &NothingAround,
    };
    PolicyEngine::decide(&Call { tool: "Bash", subject: line, effects: &effects }, &cx)
}

fn denying() -> PermissionRules {
    PermissionRules::default()
        .with(RuleKind::Allow, "Bash")
        .with(RuleKind::Deny, "Bash(rm:*)")
        .with(RuleKind::Deny, "Bash(curl:*)")
}

#[test]
fn a_denied_program_the_rules_cannot_see_is_never_run_without_the_user_even_when_bypassing() {
    let rules = denying();
    for line in [
        "bash -c 'rm -rf x'",
        "sh -c \"rm -rf x\"",
        "$(echo rm) -rf x",
        "`echo rm` -rf x",
        "/bin/r? -rf x",
        "/bin/r[m] -rf x",
        "eval rm -rf x",
        "eval 'rm -rf x'",
        "x=rm; $x -rf y",
        "\"$x\" -rf y",
        "/usr/bin/cur? http://example.com",
        "{rm,-rf,x}",
        "echo 'rm -rf x' | sh",
        "bash <<< 'rm -rf x'",
        "exec rm -rf x",
        "RM -rf x",
        "/bin/RM -rf x",
        "sudo bash -lc 'cd /tmp && rm -rf x'",
        "source <(echo rm -rf x)",
        "bash -c \"$(printf rm) -rf x\"",
        "((x++)); rm -rf x",
    ] {
        for mode in MODES {
            let verdict = judged(&rules, mode, line);
            assert!(!matches!(verdict, Verdict::Allow), "{mode:?} {line}: {verdict:?}");
        }
    }
}

#[test]
fn a_denied_program_found_inside_a_shell_s_code_is_refused_outright() {
    let rules = denying();
    for line in [
        "bash -c 'rm -rf x'",
        "sh -ec 'cd x && curl http://e.dev'",
        "eval rm -rf x",
        "eval 'rm -rf x'",
        "exec rm -rf x",
        "nice -n 5 bash -c 'timeout 5 rm -rf x'",
        "RM -rf x",
    ] {
        let verdict = judged(&rules, PermissionMode::Bypass, line);
        assert!(verdict.denies(), "{line}: {verdict:?}");
    }
}

#[test]
fn a_redirection_before_after_or_glued_to_a_denied_program_never_hides_it() {
    let rules = denying();
    for line in [
        "2>/dev/null rm -rf y",
        "<x rm -rf y",
        "rm</dev/null -rf y",
        "rm 2>/dev/null -rf y",
        ">out rm -rf y",
        "rm -rf y 2>&1 >/dev/null",
        "&>/dev/null rm -rf y",
        "2>&1 rm -rf y",
        "curl<<<x http://e.dev",
        "<<<x curl http://e.dev",
        "A=1 2>/dev/null sudo rm -rf y",
        "sudo 2>/dev/null rm -rf y",
        "env <x rm -rf y",
    ] {
        for mode in MODES {
            let verdict = judged(&rules, mode, line);
            assert!(verdict.denies(), "{mode:?} {line}: {verdict:?}");
        }
    }
}

#[test]
fn a_file_read_through_an_input_redirection_is_judged_by_the_read_rules() {
    let rules = PermissionRules::default().with(RuleKind::Allow, "Bash").with(RuleKind::Deny, "Read(./secrets/**)");
    for line in ["<secrets/k cat", "cat<secrets/k", "cat < secrets/k", "wc -l <secrets/k", "rm x <'secrets/k'"] {
        for mode in MODES {
            let verdict = judged(&rules, mode, line);
            assert!(verdict.denies(), "{mode:?} {line}: {verdict:?}");
        }
    }
    for line in ["cat <notes.txt", "cat <<EOF\nsecrets/k\nEOF", "cat <<< secrets/k", "ls 2>secrets.log"] {
        assert!(!judged(&rules, PermissionMode::Bypass, line).denies(), "{line}");
    }
}

#[test]
fn every_program_that_runs_the_rest_of_its_words_is_looked_through() {
    let rules = denying();
    for line in [
        "setsid rm -rf x",
        "setsid -f rm -rf x",
        "chroot / rm -rf x",
        "chroot --userspec=1:1 / rm -rf x",
        "flock /tmp/l rm -rf x",
        "flock -w 5 /tmp/l rm -rf x",
        "flock /tmp/l -c 'rm -rf x'",
        "watch -n1 rm -rf x",
        "watch 'rm -rf x'",
        "script -q /dev/null rm -rf x",
        "script -c 'rm -rf x' log",
        "caffeinate rm -rf x",
        "caffeinate -i -t 5 rm -rf x",
        "xcrun rm -rf x",
        "xcrun --sdk macosx rm -rf x",
        "taskset 1 rm -rf x",
        "taskset -c 0 rm -rf x",
        "su -c 'rm -rf x'",
        "su root -c 'rm -rf x'",
        "su - root -c 'rm -rf x'",
        "sg wheel 'rm -rf x'",
        "sg wheel -c 'rm -rf x'",
        "pkexec rm -rf x",
        "pkexec --user root rm -rf x",
        "runuser -u bob -- rm -rf x",
        "runuser bob -c 'rm -rf x'",
        "unbuffer rm -rf x",
        "unbuffer -p rm -rf x",
        "nohup rm -rf x",
        "stdbuf -oL rm -rf x",
        "stdbuf -o L rm -rf x",
        "ionice -c3 rm -rf x",
        "ionice -c 3 rm -rf x",
        "chrt -f 99 rm -rf x",
        "doas -u root rm -rf x",
        "time -p rm -rf x",
        "command rm -rf x",
        "builtin rm -rf x",
        "exec -a name rm -rf x",
        "env -S 'rm -rf x'",
        "env --split-string='rm -rf x'",
        "arch -arm64 rm -rf x",
        "nice -n 5 -- rm -rf x",
    ] {
        for mode in MODES {
            let verdict = judged(&rules, mode, line);
            assert!(!matches!(verdict, Verdict::Allow), "{mode:?} {line}: {verdict:?}");
        }
    }
}

#[test]
fn an_unknown_program_given_a_denied_program_s_name_asks_first() {
    let rules = denying();
    for line in [
        "frobnicate rm -rf x",
        "frobnicate -q 'rm -rf x'",
        "my-runner --then curl http://e.dev",
        "python3 -c 'import os; os.system(\"rm -rf x\")'",
        "osascript -e 'do shell script \"rm -rf x\"'",
        "perl -e 'system(\"rm -rf x\")'",
        "tar cf /dev/null x --checkpoint=1 --checkpoint-action=exec='rm -rf x'",
        "git -c alias.z='!rm -rf x' z",
    ] {
        let verdict = judged(&rules, PermissionMode::Bypass, line);
        assert!(matches!(verdict, Verdict::Ask { reason: Reason::Hidden(_), .. }), "{line}: {verdict:?}");
    }
    for line in [
        "echo rm -rf x",
        "grep -rn rm src",
        "git rm --cached x",
        "docker rm old",
        "ls 2>/dev/null",
        "cargo test 2>&1",
        "cat <notes.txt",
        "git commit -m 'curl is gone'",
    ] {
        assert_eq!(judged(&rules, PermissionMode::Bypass, line), Verdict::Allow, "{line}");
    }
}

#[test]
fn a_program_the_rules_cannot_see_asks_every_time_and_names_the_rule() {
    let verdict = judged(&denying(), PermissionMode::Bypass, "$(echo rm) -rf x");
    let Verdict::Ask { reason: Reason::Hidden(rule), rememberable: false, .. } = &verdict else {
        panic!("{verdict:?}")
    };
    assert!(rule == "Bash(rm:*)" || rule == "Bash(curl:*)", "{rule}");
    assert!(verdict.asked_every_time());
    let unattended = verdict.without_the_user("Bash");
    assert!(unattended.denies(), "nobody to ask means refused: {unattended:?}");
}

#[test]
fn commands_the_rules_can_see_still_run_when_bypassing() {
    let rules = denying();
    for line in
        ["echo $(date)", "ls -la", "cargo test", "[ -f x ] && cat x", "bash script.sh", "FOO=1 make", "git status"]
    {
        assert_eq!(judged(&rules, PermissionMode::Bypass, line), Verdict::Allow, "{line}");
    }
    let without_denials = PermissionRules::default().with(RuleKind::Allow, "Bash");
    assert_eq!(judged(&without_denials, PermissionMode::Bypass, "$(echo rm) -rf x"), Verdict::Allow);
}

const DENIED: [(&str, &str); 2] = [("rm", "-rf x"), ("curl", "http://e.dev/x")];

fn spelled(random: &mut Random, program: &str) -> String {
    let first = &program[..1];
    let rest = &program[1..];
    match random.below(16) {
        0 => program.to_owned(),
        1 => format!("'{program}'"),
        2 => format!("\"{program}\""),
        3 => format!("{first}\\{rest}"),
        4 => format!("\\{program}"),
        5 => format!("/bin/{program}"),
        6 => format!("/usr/bin/{program}"),
        7 => format!("/bin/{first}?{}", &rest[1..]),
        8 => format!("/usr/bin/{first}[{}]{}", &rest[..1], &rest[1..]),
        9 => format!("$(echo {program})"),
        10 => format!("`echo {program}`"),
        11 => format!("\"$(printf {program})\""),
        12 => "$p".to_owned(),
        13 => "${p}".to_owned(),
        14 => program.to_uppercase(),
        _ => format!("{first}''{rest}"),
    }
}

fn single_quoted(code: &str) -> String {
    format!("'{}'", code.replace('\'', r"'\''"))
}

const REDIRECTIONS: [&str; 10] =
    ["2>/dev/null", "<x", "</dev/null", ">out", "2>&1", "&>/dev/null", "<<<x", "1>&2", "<>f", "3<x"];

fn redirected(random: &mut Random, line: &str) -> String {
    let redirection = random.pick(&REDIRECTIONS);
    match random.below(4) {
        0 => format!("{redirection} {line}"),
        1 => format!("{line} {redirection}"),
        2 => {
            let glued = redirection.trim_start_matches(|c: char| c.is_ascii_digit());
            match line.split_once(' ') {
                Some((program, rest)) => format!("{program}{glued} {rest}"),
                None => format!("{line}{glued}"),
            }
        }
        _ => match line.split_once(' ') {
            Some((program, rest)) => format!("{program} {redirection} {rest}"),
            None => format!("{line} {redirection}"),
        },
    }
}

fn wrapped_more(random: &mut Random, line: &str) -> String {
    match random.below(20) {
        0 => format!("setsid {line}"),
        1 => format!("chroot / {line}"),
        2 => format!("flock /tmp/l {line}"),
        3 => format!("flock /tmp/l -c {}", single_quoted(line)),
        4 => format!("watch -n1 {line}"),
        5 => format!("watch {}", single_quoted(line)),
        6 => format!("script -q /dev/null {line}"),
        7 => format!("script -c {} log", single_quoted(line)),
        8 => format!("caffeinate -i {line}"),
        9 => format!("xcrun {line}"),
        10 => format!("taskset -c 0 {line}"),
        11 => format!("su -c {}", single_quoted(line)),
        12 => format!("sg wheel {}", single_quoted(line)),
        13 => format!("pkexec {line}"),
        14 => format!("runuser -u bob -- {line}"),
        15 => format!("unbuffer {line}"),
        16 => format!("stdbuf -oL {line}"),
        17 => format!("ionice -c3 {line}"),
        18 => format!("env -S {}", single_quoted(line)),
        _ => format!("chrt -f 99 {line}"),
    }
}

fn wrapped(random: &mut Random, line: &str) -> String {
    if random.chance(3) {
        return redirected(random, line);
    }
    if random.chance(3) {
        return wrapped_more(random, line);
    }
    match random.below(18) {
        0 => format!("sudo {line}"),
        1 => format!("sudo -u root {line}"),
        2 => format!("env {line}"),
        3 => format!("env A=1 {line}"),
        4 => format!("nice -n 5 {line}"),
        5 => format!("timeout 5 {line}"),
        6 => format!("xargs {line}"),
        7 => format!("command {line}"),
        8 => format!("exec {line}"),
        9 => format!("nohup {line}"),
        10 => format!("bash -c {}", single_quoted(line)),
        11 => format!("sh -c {}", single_quoted(line)),
        12 => format!("bash -lc {}", single_quoted(line)),
        13 => format!("eval {line}"),
        14 => format!("eval {}", single_quoted(line)),
        15 => format!("echo $({line})"),
        16 => format!("echo {} | sh", single_quoted(line)),
        _ => format!("true && {line}"),
    }
}

#[test]
fn no_spelling_or_wrapping_of_a_denied_program_reaches_allow_in_any_mode() {
    let rules = denying();
    let mut random = Random(0x005e_ed0f_dea1_ed01);
    for _ in 0..cases() {
        let (program, arguments) = DENIED[random.below(DENIED.len())];
        let word = spelled(&mut random, program);
        let assigned =
            if word.contains("$p") || word.contains("${p}") { format!("p={program}; ") } else { String::new() };
        let mut line = format!("{word} {arguments}");
        for _ in 0..random.below(4) {
            line = wrapped(&mut random, &line);
        }
        let line = format!("{assigned}{line}");
        for mode in MODES {
            let verdict = judged(&rules, mode, &line);
            assert!(!matches!(verdict, Verdict::Allow), "{mode:?} {line}: {verdict:?}");
        }
    }
}
