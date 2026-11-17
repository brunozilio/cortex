#![no_main]

use domain::permission_rules::{
    PermissionRules, RuleKind, Target, hides_more, normalized, split_command, without_harmless_redirects,
};
use domain::shell::{ShellCommand, SimpleCommand, simple_commands};
use libfuzzer_sys::fuzz_target;

const CWD: &str = "/work/project";
const HOME: Option<&str> = Some("/home/user");

fn restricting() -> PermissionRules {
    PermissionRules::default()
        .with(RuleKind::Allow, "Bash")
        .with(RuleKind::Deny, "Bash(rm:*)")
        .with(RuleKind::Ask, "Bash(git push:*)")
}

fn narrow() -> PermissionRules {
    PermissionRules::default().with(RuleKind::Allow, "Bash(git status:*)").with(RuleKind::Allow, "Bash(echo:*)")
}

fuzz_target!(|data: &[u8]| {
    let line = String::from_utf8_lossy(data);
    let line = line.as_ref();

    let parsed = ShellCommand::parse(line);
    assert_eq!(parsed.text(), line);
    for command in parsed.commands() {
        assert!(line.contains(command), "a command is a piece of the line it came from");
        let _ = SimpleCommand::of(command);
        let _ = SimpleCommand::bare(command);
    }
    if parsed.flat() {
        assert!(!parsed.opaque());
    }
    let (commands, understood) = simple_commands(line);
    assert_eq!(commands, parsed.commands());
    assert_eq!(understood, !parsed.opaque());

    let (parts, _) = split_command(line);
    let rules = restricting();
    let target = Target::Command(line);
    let decided = rules.decide("Bash", &target, CWD, HOME);
    let _ = rules.unseen_denial("Bash", &target);
    for part in &parts {
        let _ = hides_more(part);
        let _ = without_harmless_redirects(part);
        let run = normalized(part);
        if decided == Some(RuleKind::Allow) {
            let program = run.split(' ').next().unwrap_or_default();
            assert_ne!(program, "rm", "a line whose part runs rm was allowed: {line:?}");
        }
    }

    let _ = narrow().ruling("Bash", &target, CWD, HOME);
});
