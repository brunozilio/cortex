use super::readers::{READERS, Reader};
use super::*;
use crate::permission_rules::{PermissionRules, RuleKind, Target};
use crate::shell::sample::{Random, cases};

const WRAPPERS: [&str; 10] =
    ["sudo", "env", "nice", "nohup", "time", "command", "xargs", "busybox", "doas", "timeout 5"];
const PROGRAMS: [&str; 18] = [
    "ls",
    "cat",
    "grep",
    "find",
    "git status",
    "git diff",
    "git log",
    "head",
    "wc",
    "rg",
    "sort",
    "rm",
    "cargo build",
    "git push",
    "curl",
    "touch",
    "tee",
    "printenv",
];
const ARGUMENTS: [&str; 30] = [
    "-l",
    "-la",
    "-n",
    "-rf",
    "--output=x",
    "-o",
    "-delete",
    "-exec",
    "--force",
    "-f",
    "-name",
    "-c",
    "--stat",
    "a.txt",
    "src",
    "'*.rs'",
    "*.rs",
    "$HOME",
    "../x",
    "/etc/hosts",
    "~/x",
    ".env",
    "HEAD~1",
    "\"q w\"",
    "x~",
    "id_rsa",
    "{a,b}",
    "'a.*b'",
    "$(id)",
    "`id`",
];
const REDIRECTS: [&str; 6] = [">out.txt", "2>/dev/null", ">>log", "<in.txt", "2>&1", "&>all"];
const JOINS: [&str; 5] = [" && ", " || ", "; ", " | ", "\n"];

fn simple(random: &mut Random) -> String {
    let mut words: Vec<String> = Vec::new();
    if random.chance(6) {
        words.push("X=1".into());
    }
    if random.chance(6) {
        words.push(random.pick(&WRAPPERS).into());
    }
    words.push(random.pick(&PROGRAMS).into());
    for _ in 0..random.below(4) {
        words.push(random.pick(&ARGUMENTS).into());
    }
    if random.chance(5) {
        words.push(random.pick(&REDIRECTS).into());
    }
    words.join(" ")
}

fn line(random: &mut Random) -> String {
    let mut line = simple(random);
    for _ in 0..random.below(3) {
        line.push_str(random.pick(&JOINS));
        line.push_str(&simple(random));
    }
    line
}

fn reader_of(words: &[String]) -> Option<&'static Reader> {
    READERS
        .iter()
        .filter(|reader| reader.program.iter().zip(words).all(|(program, word)| program == word))
        .filter(|reader| words.len() >= reader.program.len())
        .max_by_key(|reader| reader.program.len())
}

fn listed(reader: &Reader, flag: &str) -> bool {
    let name = flag.split_once('=').map_or(flag, |(name, _)| name);
    if reader.flags.contains(&name) || reader.flags.contains(&format!("{name}=").as_str()) {
        return true;
    }
    if !reader.clustered || flag.starts_with("--") {
        return false;
    }
    for c in flag.chars().skip(1).filter(|c| !c.is_ascii_digit()) {
        if reader.flags.contains(&format!("-{c}=").as_str()) {
            return true;
        }
        if !reader.flags.contains(&format!("-{c}").as_str()) {
            return false;
        }
    }
    true
}

fn assert_only_reads(line: &str) {
    for part in split_command(line).0 {
        let written = without_harmless_redirects(part);
        assert!(!written.contains('>'), "{line:?}: a read-only command writes through {part:?}");
        let words = words(&written);
        let first = words.first().map(String::as_str).unwrap_or_default();
        assert!(!first.contains('='), "{line:?}: a read-only command changes the environment");
        assert!(!WRAPPERS.iter().any(|wrapper| wrapper.split(' ').next() == Some(first)), "{line:?}: wrapped");
        let reader = reader_of(&words).unwrap_or_else(|| panic!("{line:?}: {part:?} is no listed reader"));
        let flags = words[reader.program.len()..].iter().take_while(|word| *word != "--");
        for flag in flags.filter(|word| word.starts_with('-') && *word != "-") {
            assert!(listed(reader, flag), "{line:?}: {flag} is not a listed flag of {:?}", reader.program);
        }
    }
}

#[test]
fn a_read_only_command_writes_nothing_changes_no_environment_and_uses_only_listed_flags() {
    let mut random = Random(0x2545_f491_4f6c_dd1d);
    let mut read_only = 0;
    for _ in 0..cases() {
        let line = line(&mut random);
        if classify(&line, &GitFacts::default(), &nowhere) == CommandClass::ReadOnly {
            read_only += 1;
            assert_only_reads(&line);
        }
    }
    assert!(read_only > 0 || cases() < 100, "the generator should reach some read-only commands");
}

fn rules() -> PermissionRules {
    PermissionRules::default()
        .with(RuleKind::Allow, "Bash")
        .with(RuleKind::Deny, "Bash(rm:*)")
        .with(RuleKind::Deny, "Bash(git push:*)")
        .with(RuleKind::Deny, "Bash(curl:*)")
}

fn ruled(rules: &PermissionRules, line: &str) -> Option<RuleKind> {
    rules.decide("Bash", &Target::Command(line), "/work/rocket", Some("/home/ana"))
}

#[test]
fn a_wrapper_or_a_control_structure_never_lifts_a_denial() {
    let (rules, plain) = (rules(), GitFacts::default());
    let mut random = Random(0x1234_5678_9abc_def1);
    for _ in 0..cases() {
        let line = line(&mut random);
        let ruling = ruled(&rules, &line);
        let class = classify(&line, &plain, &nowhere);
        for wrapped in [format!("sudo {line}"), format!("env X=1 {line}"), format!("if true; then {line}\nfi")] {
            if ruling == Some(RuleKind::Deny) {
                let now = ruled(&rules, &wrapped);
                assert!(matches!(now, Some(RuleKind::Deny | RuleKind::Ask)), "{wrapped:?} is {now:?}");
            }
            let now = classify(&wrapped, &plain, &nowhere);
            if matches!(class, CommandClass::Forbidden(_)) {
                assert!(matches!(now, CommandClass::Forbidden(_)), "{wrapped:?} is {now:?}");
            }
            if class != CommandClass::ReadOnly {
                assert_ne!(now, CommandClass::ReadOnly, "{wrapped:?}");
            }
        }
    }
}
