#![no_main]

use arbitrary::{Result, Unstructured};
use domain::permission_rules::{PermissionRules, RuleKind, Target};
use libfuzzer_sys::fuzz_target;

const CWD: &str = "/work/project";
const HOME: Option<&str> = Some("/home/user");

const KEYWORDS: [&str; 22] = [
    "if", "then", "else", "elif", "fi", "case", "esac", "for", "select", "while", "until", "do", "done", "in",
    "function", "time", "coproc", "!", "{", "}", "[[", "]]",
];
const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789-._/+,:@%";
const SPELLINGS: [&str; 9] = ["rm", "\"rm\"", "'rm'", "\\rm", "/bin/rm", "r\"m\"", "/usr/bin/rm", "r''m", "RM"];
const WRAPPERS: [&str; 10] = [
    "stdbuf -oL ",
    "sudo ",
    "env ",
    "nice -n 5 ",
    "timeout 5 ",
    "time ",
    "command ",
    "nohup ",
    "sudo -u root ",
    "env -i PATH=/bin ",
];
const GROUPS: [(&str, &str); 11] = [
    ("( ", " )"),
    ("{ ", "; }"),
    ("$( ", " )"),
    ("echo \"$(", ")\""),
    ("if true; then ", "; fi"),
    ("if ", "; then :; fi"),
    ("while ", "; do :; done"),
    ("until ", "; do :; done"),
    ("for f in a b; do ", "; done"),
    ("! ", ""),
    ("true && ", ""),
];
const SEPARATORS: [&str; 7] = ["; ", " && ", " || ", " | ", "\n", " & ", ";"];

fn word(u: &mut Unstructured<'_>) -> Result<String> {
    let length = u.int_in_range(1..=8)?;
    let mut word = String::new();
    for _ in 0..length {
        word.push(char::from(*u.choose(LETTERS)?));
    }
    if KEYWORDS.contains(&word.as_str()) {
        word.insert(0, 'x');
    }
    if word.eq_ignore_ascii_case("rm") {
        word.push('x');
    }
    Ok(word)
}

fn filler(u: &mut Unstructured<'_>) -> Result<String> {
    let count = u.int_in_range(1..=4)?;
    let words = (0..count).map(|_| word(u)).collect::<Result<Vec<_>>>()?;
    Ok(words.join(" "))
}

fn fillers(u: &mut Unstructured<'_>) -> Result<Vec<String>> {
    let count = u.int_in_range(0..=3)?;
    (0..count).map(|_| filler(u)).collect()
}

fn joined(u: &mut Unstructured<'_>, commands: &[String]) -> Result<String> {
    let mut line = String::new();
    for (at, command) in commands.iter().enumerate() {
        if at > 0 {
            line.push_str(u.choose(&SEPARATORS)?);
        }
        line.push_str(command);
    }
    Ok(line)
}

fn denied(u: &mut Unstructured<'_>) -> Result<String> {
    let mut command = (*u.choose(&SPELLINGS)?).to_owned();
    for _ in 0..u.int_in_range(0..=3)? {
        command.push(' ');
        command.push_str(&word(u)?);
    }
    for _ in 0..u.int_in_range(0..=2)? {
        command.insert_str(0, u.choose(&WRAPPERS)?);
    }
    if u.arbitrary()? {
        command.insert_str(0, "VAR=1 ");
    }
    for _ in 0..u.int_in_range(0..=3)? {
        let (open, close) = u.choose(&GROUPS)?;
        command = format!("{open}{command}{close}");
    }
    Ok(command)
}

fn line(u: &mut Unstructured<'_>) -> Result<String> {
    let mut commands = fillers(u)?;
    let at = u.int_in_range(0..=commands.len())?;
    commands.insert(at, denied(u)?);
    joined(u, &commands)
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(line) = line(&mut u) else { return };
    let rules = PermissionRules::default().with(RuleKind::Allow, "Bash").with(RuleKind::Deny, "Bash(rm:*)");
    let decided = rules.decide("Bash", &Target::Command(&line), CWD, HOME);
    assert_ne!(decided, Some(RuleKind::Allow), "a line that runs rm was allowed: {line:?}");
});
