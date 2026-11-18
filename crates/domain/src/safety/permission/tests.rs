use std::path::PathBuf;

use super::*;
use crate::PermissionMode;
use crate::permission_rules::{PermissionRules, RuleKind};

const CWD: &str = "/work/rocket";

fn bounds() -> Bounds {
    Bounds::of(vec![PathBuf::from(CWD)], vec![PathBuf::from("/data/results")])
}

fn context<'a>(
    mode: PermissionMode,
    rules: &'a PermissionRules,
    remembered: &'a Remembered,
    bounds: &'a Bounds,
) -> PolicyContext<'a> {
    PolicyContext {
        mode,
        ceiling: None,
        rules,
        remembered,
        bounds,
        cwd: CWD,
        home: Some("/home/ana"),
        sandboxed: false,
        sandbox_missing: None,
        no_prompts: false,
        surroundings: &NothingAround,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Legacy {
    Reads,
    ReadsOutside,
    Edits,
    EditsGuarded,
    Executes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Allow,
    Ask,
    Deny,
}

fn legacy_by_mode(mode: PermissionMode, access: Legacy) -> Outcome {
    match (mode, access) {
        (_, Legacy::Reads) | (PermissionMode::Bypass, _) => Outcome::Allow,
        (_, Legacy::ReadsOutside) => Outcome::Ask,
        (PermissionMode::Plan, _) => Outcome::Deny,
        (PermissionMode::AcceptEdits, Legacy::Edits) => Outcome::Allow,
        _ => Outcome::Ask,
    }
}

fn legacy(mode: PermissionMode, access: Legacy, rule: Option<RuleKind>) -> Outcome {
    let by_mode = legacy_by_mode(mode, access);
    let planning =
        mode == PermissionMode::Plan && matches!(access, Legacy::Edits | Legacy::Executes | Legacy::EditsGuarded);
    match rule {
        Some(RuleKind::Deny) => Outcome::Deny,
        _ if mode == PermissionMode::Bypass || planning => by_mode,
        Some(RuleKind::Ask) => Outcome::Ask,
        Some(RuleKind::Allow) => Outcome::Allow,
        None => by_mode,
    }
}

fn outcome(verdict: &Verdict) -> Outcome {
    match verdict {
        Verdict::Allow => Outcome::Allow,
        Verdict::Ask { .. } => Outcome::Ask,
        Verdict::Deny(_) => Outcome::Deny,
    }
}

struct Case {
    tool: &'static str,
    subject: &'static str,
    effects: Vec<Effect>,
    access: Legacy,
    bare_allow_counts: bool,
}

fn place(path: &str) -> Place {
    Place::at(path)
}

fn cases() -> Vec<Case> {
    let case = |tool, subject, effects, access| Case { tool, subject, effects, access, bare_allow_counts: true };
    let guarded = |tool, path: &'static str| Case {
        tool,
        subject: path,
        effects: vec![Effect::WritePath(place(path))],
        access: Legacy::EditsGuarded,
        bare_allow_counts: false,
    };
    vec![
        case("Read", "src/a.rs", vec![Effect::ReadPath(place("/work/rocket/src/a.rs"))], Legacy::Reads),
        case("Read", "/etc/hosts", vec![Effect::ReadPath(place("/etc/hosts"))], Legacy::ReadsOutside),
        case("Read", "saved", vec![Effect::ReadPath(place("/data/results/call_1.txt"))], Legacy::Reads),
        case("Grep", "todo", vec![Effect::ReadTree(place("/work/rocket/src"))], Legacy::Reads),
        case("Grep", "todo", vec![Effect::ReadTree(place("/opt/lib"))], Legacy::ReadsOutside),
        case("LSP", "hover a.rs", vec![Effect::ReadPath(place("/work/rocket/a.rs"))], Legacy::Reads),
        case("Write", "src/a.rs", vec![Effect::WritePath(place("/work/rocket/src/a.rs"))], Legacy::Edits),
        case("Edit", "src/a.rs", vec![Effect::WritePath(place("/work/rocket/src/a.rs"))], Legacy::Edits),
        case("NotebookEdit", "n.ipynb", vec![Effect::WritePath(place("/work/rocket/n.ipynb"))], Legacy::Edits),
        guarded("Write", "/home/ana/.zshrc"),
        guarded("Edit", "/work/rocket/.git/config"),
        guarded("Edit", "/work/rocket/sub/.GIT/hooks/pre-commit"),
        Case {
            tool: "Write",
            subject: "settings",
            effects: vec![Effect::WritePath(place(&format!("/work/rocket/{}/settings.json", brand::DIR)))],
            access: Legacy::EditsGuarded,
            bare_allow_counts: false,
        },
        case("Bash", "ls -la", vec![Effect::exec("ls -la", false)], Legacy::Reads),
        case("Bash", "cargo build", vec![Effect::exec("cargo build", false)], Legacy::Executes),
        case("WebFetch", "https://docs.rs/x", vec![Effect::Net { host: Some("docs.rs".into()) }], Legacy::Executes),
        case("WebSearch", "rust", vec![Effect::Net { host: None }], Legacy::Executes),
        case("Agent", "explore", vec![Effect::Spawn(SpawnKind::Agent)], Legacy::Reads),
        case("CronCreate", "nightly", vec![Effect::Spawn(SpawnKind::Schedule)], Legacy::Executes),
        case("PushNotification", "done", vec![Effect::Notify], Legacy::Reads),
        case("SendMessage", "to session web", vec![Effect::Session { to: "web".into() }], Legacy::Executes),
        case("mcp__jira__search", "bug", vec![Effect::Remote { server: "jira".into() }], Legacy::Executes),
        case("Deploy", "prod", vec![Effect::Declared(Declared::Executes)], Legacy::Executes),
        case("Format", "a.rs", vec![Effect::Declared(Declared::Edits)], Legacy::Edits),
        case("TaskList", "", vec![], Legacy::Reads),
    ]
}

#[test]
fn every_tool_in_every_mode_under_every_kind_of_rule_gets_the_mode_table_s_verdict() {
    let none = Remembered::default();
    let bounds = bounds();
    for case in cases() {
        for mode in PermissionMode::ALL {
            for kind in [None, Some(RuleKind::Allow), Some(RuleKind::Ask), Some(RuleKind::Deny)] {
                let rules =
                    kind.map_or_else(PermissionRules::default, |kind| PermissionRules::default().with(kind, case.tool));
                let counted = kind.filter(|kind| *kind != RuleKind::Allow || case.bare_allow_counts);
                let call = Call { tool: case.tool, subject: case.subject, effects: &case.effects };
                let verdict = PolicyEngine::decide(&call, &context(mode, &rules, &none, &bounds));
                assert_eq!(
                    outcome(&verdict),
                    legacy(mode, case.access, counted),
                    "{} {:?} {mode:?} {kind:?}: {verdict:?}",
                    case.tool,
                    case.effects
                );
            }
        }
    }
}

fn decide(tool: &str, subject: &str, effects: &[Effect], mode: PermissionMode, rules: &PermissionRules) -> Verdict {
    let none = Remembered::default();
    let bounds = bounds();
    PolicyEngine::decide(&Call { tool, subject, effects }, &context(mode, rules, &none, &bounds))
}

#[test]
fn a_rule_naming_its_target_is_matched_against_the_effect() {
    let ask = PermissionMode::Ask;
    let rules = PermissionRules::default()
        .with(RuleKind::Deny, "Read(secrets/**)")
        .with(RuleKind::Allow, "Edit(docs/**)")
        .with(RuleKind::Allow, "Bash(cargo test:*)")
        .with(RuleKind::Allow, "WebFetch(domain:docs.rs)")
        .with(RuleKind::Allow, "Edit(.git/info/exclude)")
        .with(RuleKind::Deny, "mcp__docs__resources");
    let read = [Effect::ReadPath(place("/work/rocket/secrets/key"))];
    assert!(decide("Read", "k", &read, ask, &rules).denies());
    let search = [Effect::ReadTree(place(CWD))];
    assert!(decide("Grep", "k", &search, ask, &rules).denies(), "the tree holds the denied folder");
    let docs = [Effect::WritePath(place("/work/rocket/docs/a.md"))];
    assert_eq!(decide("Write", "a", &docs, ask, &rules), Verdict::Allow);
    let test = [Effect::exec("cargo test -p cli", false)];
    assert_eq!(decide("Bash", "cargo test -p cli", &test, ask, &rules), Verdict::Allow);
    let fetch = [Effect::Net { host: Some("docs.rs".into()) }];
    assert_eq!(decide("WebFetch", "https://docs.rs/a", &fetch, ask, &rules), Verdict::Allow);
    let elsewhere = [Effect::Net { host: Some("evil.dev".into()) }];
    assert!(decide("WebFetch", "https://evil.dev", &elsewhere, ask, &rules).asks());
    let exclude = [Effect::WritePath(place("/work/rocket/.git/info/exclude"))];
    assert_eq!(decide("Edit", "x", &exclude, ask, &rules), Verdict::Allow, "a rule naming the guarded path");
    let resource = [Effect::Resource { server: "docs".into(), uri: "docs://intro".into() }];
    assert!(decide("ReadMcpResource", "docs://intro", &resource, ask, &rules).denies());
}

#[test]
fn a_link_is_judged_as_written_and_where_it_leads() {
    let rules =
        PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)").with(RuleKind::Allow, "Edit(src/**)");
    let linked = Place { written: "/work/rocket/src/link/key".into(), real: "/work/rocket/secrets/key".into() };
    let read = [Effect::ReadPath(linked.clone())];
    assert!(decide("Read", "k", &read, PermissionMode::Ask, &rules).denies());
    let write = [Effect::WritePath(linked)];
    assert!(decide("Edit", "k", &write, PermissionMode::Ask, &rules).asks(), "allowed as written only");
}

#[test]
fn the_verdict_says_why_it_asks_and_what_always_would_remember() {
    let none = PermissionRules::default();
    let outside = [Effect::ReadPath(place("/home/ana/notes/todo.md"))];
    assert_eq!(
        decide("Read", "todo", &outside, PermissionMode::Plan, &none),
        Verdict::Ask {
            reason: Reason::Outside("/home/ana/notes/todo.md".into()),
            rememberable: true,
            rule: Some("Read(//home/ana/notes/**)".into())
        }
    );
    let git = [Effect::WritePath(place("/work/rocket/.git/config"))];
    assert_eq!(
        decide("Edit", ".git/config", &git, PermissionMode::AcceptEdits, &none),
        Verdict::Ask {
            reason: Reason::Guarded { path: "/work/rocket/.git/config".into(), guard: Guard::GitFolder },
            rememberable: true,
            rule: None
        }
    );
    let build = [Effect::exec("cargo build --release", false)];
    assert_eq!(
        decide("Bash", "cargo build --release", &build, PermissionMode::AcceptEdits, &none),
        Verdict::Ask { reason: Reason::Mode, rememberable: true, rule: Some("Bash(cargo build:*)".into()) }
    );
    let asking = PermissionRules::default().with(RuleKind::Ask, "Edit(docs/**)");
    let docs = [Effect::WritePath(place("/work/rocket/docs/a.md"))];
    assert!(decide("Edit", "a", &docs, PermissionMode::AcceptEdits, &asking).asked_by_a_rule());
    assert_eq!(decide("Edit", "a", &docs, PermissionMode::Bypass, &asking), Verdict::Allow);
}

#[test]
fn a_risky_command_or_one_leaving_the_sandbox_asks_every_time_and_secrets_are_refused() {
    let open = PermissionRules::default().with(RuleKind::Allow, "Bash");
    let push = [Effect::exec("git push --force", false)];
    let verdict = decide("Bash", "git push --force", &push, PermissionMode::Bypass, &open);
    assert!(verdict.asked_every_time(), "{verdict:?}");
    assert!(matches!(&verdict, Verdict::Ask { rememberable: false, reason: Reason::Risky(_), .. }), "{verdict:?}");
    let denied = PermissionRules::default().with(RuleKind::Deny, "Bash(git push:*)");
    assert!(decide("Bash", "git push --force", &push, PermissionMode::Bypass, &denied).denies());
    let secret = [Effect::exec("cat .env", false)];
    assert!(decide("Bash", "cat .env", &secret, PermissionMode::Bypass, &open).denies());

    let (none, bounds) = (Remembered::default(), bounds());
    let sandboxed = PolicyContext { sandboxed: true, ..context(PermissionMode::Bypass, &open, &none, &bounds) };
    let leaving = [Effect::exec("cargo install x", true)];
    let call = Call { tool: "Bash", subject: "cargo install x", effects: &leaving };
    assert!(matches!(
        PolicyEngine::decide(&call, &sandboxed),
        Verdict::Ask { reason: Reason::LeavesTheSandbox, rememberable: false, .. }
    ));
    let unattended = PolicyContext { no_prompts: true, ..sandboxed };
    let settled = unattended.settled(PolicyEngine::decide(&call, &unattended), "Bash");
    assert!(matches!(settled, Verdict::Deny(why) if why.contains("leaving the sandbox")));
}

#[test]
fn a_required_sandbox_the_system_cannot_provide_refuses_every_confined_command() {
    let (open, none, bounds) = (PermissionRules::default(), Remembered::default(), bounds());
    let missing = PolicyContext {
        sandbox_missing: Some("this kernel has no Landlock"),
        ..context(PermissionMode::Bypass, &open, &none, &bounds)
    };
    let decide = |effects: &[Effect], subject| PolicyEngine::decide(&Call { tool: "Bash", subject, effects }, &missing);
    let listing = [Effect::exec("ls", false)];
    assert!(matches!(decide(&listing, "ls"), Verdict::Deny(why) if why.contains("this kernel has no Landlock")));
    let reading = [Effect::ReadPath(place("/work/rocket/a.rs"))];
    assert_eq!(
        PolicyEngine::decide(&Call { tool: "Read", subject: "a.rs", effects: &reading }, &missing),
        Verdict::Allow
    );
    let leaving = [Effect::exec("cargo install x", true)];
    assert!(
        matches!(decide(&leaving, "cargo install x"), Verdict::Ask { reason: Reason::LeavesTheSandbox, .. }),
        "running outside it is still the user's call, every time"
    );
}

#[test]
fn always_is_remembered_by_tool_for_edits_and_by_subject_for_the_rest() {
    let (rules, bounds) = (PermissionRules::default(), bounds());
    let mut remembered = Remembered::default();
    remembered.remember("Edit", Standing::Edits, "a.rs");
    remembered.remember("Bash", Standing::Acts, "cargo test");
    remembered.remember("Read", Standing::ReadsOutside, "/etc/hosts");
    remembered.allow_rule("WebFetch(domain:docs.rs)");
    let cx = context(PermissionMode::Ask, &rules, &remembered, &bounds);
    let decide = |tool, subject, effects: &[Effect]| PolicyEngine::decide(&Call { tool, subject, effects }, &cx);
    assert_eq!(decide("Edit", "b.rs", &[Effect::WritePath(place("/work/rocket/b.rs"))]), Verdict::Allow);
    assert_eq!(decide("Bash", "cargo test", &[Effect::exec("cargo test", false)]), Verdict::Allow);
    assert!(decide("Bash", "rm -rf x", &[Effect::exec("rm -rf x", false)]).asks());
    assert_eq!(decide("Read", "h", &[Effect::ReadPath(place("/etc/hosts"))]), Verdict::Allow);
    assert!(decide("Read", "p", &[Effect::ReadPath(place("/etc/passwd"))]).asks(), "by path");
    assert!(decide("Edit", "c", &[Effect::WritePath(place("/work/rocket/.git/config"))]).asks(), "guarded by path");
    assert_eq!(decide("WebFetch", "u", &[Effect::Net { host: Some("docs.rs".into()) }]), Verdict::Allow);
}

#[test]
fn a_sub_agent_s_ceiling_caps_the_mode_the_engine_decides_in() {
    let (rules, none, bounds) = (PermissionRules::default(), Remembered::default(), bounds());
    let capped = PolicyContext {
        ceiling: Some(PermissionMode::Plan),
        ..context(PermissionMode::Bypass, &rules, &none, &bounds)
    };
    let write = [Effect::WritePath(place("/work/rocket/a.rs"))];
    assert!(PolicyEngine::decide(&Call { tool: "Write", subject: "a.rs", effects: &write }, &capped).denies());
}

#[test]
fn a_bash_rule_restricts_a_command_another_tool_runs_but_does_not_allow_it() {
    let rules = PermissionRules::default().with(RuleKind::Deny, "Bash(rm:*)").with(RuleKind::Allow, "Bash(tail:*)");
    let watch = |command: &str| {
        let effects = [Effect::exec(command, false), Effect::Spawn(SpawnKind::BackgroundShell)];
        decide("Monitor", command, &effects, PermissionMode::AcceptEdits, &rules)
    };
    assert!(watch("rm -rf build").denies());
    assert!(watch("tail -f log.txt").asks(), "Monitor has no allow rule of its own");
    assert!(watch("cat .env").denies(), "secrets are refused whoever runs the command");
}

#[test]
fn stopping_or_announcing_its_own_work_is_free_and_reaching_beyond_it_asks() {
    let none = PermissionRules::default();
    let ask = PermissionMode::Ask;
    let decide_one = |tool, effect: Effect, mode, rules: &PermissionRules| decide(tool, "", &[effect], mode, rules);
    assert_eq!(decide_one("KillShell", Effect::Cancel { own: true }, ask, &none), Verdict::Allow);
    assert!(decide_one("KillShell", Effect::Cancel { own: false }, ask, &none).asks(), "a shell it did not start");
    assert_eq!(decide_one("PushNotification", Effect::Notify, ask, &none), Verdict::Allow);
    assert_eq!(decide_one("CronDelete", Effect::Cancel { own: true }, ask, &none), Verdict::Allow);
    assert_eq!(decide_one("TaskStop", Effect::Cancel { own: true }, ask, &none), Verdict::Allow);
    assert_eq!(decide_one("Agent", Effect::Spawn(SpawnKind::Agent), ask, &none), Verdict::Allow);
    let to = || Effect::Session { to: "web-9402".into() };
    assert!(decide_one("SendMessage", to(), ask, &none).asks(), "another session");
    let allowed = PermissionRules::default().with(RuleKind::Allow, "SendMessage");
    assert_eq!(decide_one("SendMessage", to(), ask, &allowed), Verdict::Allow);
    let resource = || Effect::Resource { server: "docs".into(), uri: "docs://intro".into() };
    assert!(decide_one("ReadMcpResource", resource(), ask, &none).asks());
    let server = PermissionRules::default().with(RuleKind::Allow, "mcp__docs");
    assert_eq!(decide_one("ReadMcpResource", resource(), ask, &server), Verdict::Allow, "the server's rule");
}

#[test]
fn planning_allows_what_only_reads_or_tends_its_own_work_and_refuses_the_rest() {
    let (none, plan) = (PermissionRules::default(), PermissionMode::Plan);
    let decide_one = |tool, effect: Effect| decide(tool, "", &[effect], plan, &none);
    for (tool, effect) in [
        ("Read", Effect::ReadPath(place("/work/rocket/a.rs"))),
        ("Grep", Effect::ReadTree(place(CWD))),
        ("Bash", Effect::exec("git log --oneline -5", false)),
        ("PushNotification", Effect::Notify),
        ("KillShell", Effect::Cancel { own: true }),
        ("Agent", Effect::Spawn(SpawnKind::Agent)),
        ("Watch", Effect::Spawn(SpawnKind::Watch)),
    ] {
        assert_eq!(decide_one(tool, effect), Verdict::Allow, "{tool}");
    }
    for (tool, effect) in [
        ("Write", Effect::WritePath(place("/work/rocket/a.rs"))),
        ("Bash", Effect::exec("cargo build", false)),
        ("WebFetch", Effect::Net { host: Some("docs.rs".into()) }),
        ("SendMessage", Effect::Session { to: "web".into() }),
        ("CronCreate", Effect::Spawn(SpawnKind::Schedule)),
        ("KillShell", Effect::Cancel { own: false }),
        ("mcp__jira__create", Effect::Remote { server: "jira".into() }),
        ("Deploy", Effect::Declared(Declared::Executes)),
    ] {
        assert!(decide_one(tool, effect).denies(), "{tool}");
    }
    assert!(decide_one("Read", Effect::ReadPath(place("/etc/hosts"))).asks(), "reading outside still asks");
}

fn installing() -> [Effect; 2] {
    [
        Effect::ReadPath(place("/work/rocket/main.rs")),
        Effect::Install { server: "rust-analyzer".into(), what: "download https://example.invalid/ra.gz".into() },
    ]
}

#[test]
fn installing_a_language_server_is_asked_in_every_mode_but_plan_which_refuses_it() {
    let open = PermissionRules::default().with(RuleKind::Allow, "LSP");
    for mode in [PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Bypass] {
        for rules in [&PermissionRules::default(), &open] {
            let verdict = decide("LSP", "hover main.rs", &installing(), mode, rules);
            assert!(
                matches!(&verdict, Verdict::Ask { reason: Reason::Install { what, .. }, rule: Some(rule), .. }
                    if what.contains("ra.gz") && rule == "LSP(install:rust-analyzer)"),
                "{mode:?}: {verdict:?}"
            );
            assert!(verdict.asked_every_time(), "no hook answers it for the user");
        }
    }
    let planning = decide("LSP", "hover main.rs", &installing(), PermissionMode::Plan, &PermissionRules::default());
    assert!(matches!(&planning, Verdict::Deny(why) if why.contains("planning")), "{planning:?}");
    let reading = [Effect::ReadPath(place("/work/rocket/main.rs"))];
    assert_eq!(decide("LSP", "hover", &reading, PermissionMode::Plan, &PermissionRules::default()), Verdict::Allow);
}

#[test]
fn an_install_nobody_can_be_asked_about_is_refused_and_says_which_rule_allows_it() {
    let (rules, none, bounds) = (PermissionRules::default(), Remembered::default(), bounds());
    let unattended = PolicyContext { no_prompts: true, ..context(PermissionMode::Bypass, &rules, &none, &bounds) };
    let effects = installing();
    let call = Call { tool: "LSP", subject: "hover main.rs", effects: &effects };
    let settled = unattended.settled(PolicyEngine::decide(&call, &unattended), "LSP");
    assert!(
        matches!(&settled, Verdict::Deny(why) if why.contains("LSP(install:rust-analyzer)") && why.contains("permissions.allow")),
        "{settled:?}"
    );

    let named = PermissionRules::default().with(RuleKind::Allow, "LSP(install:rust-analyzer)");
    let allowed = PolicyContext { no_prompts: true, ..context(PermissionMode::Bypass, &named, &none, &bounds) };
    assert_eq!(PolicyEngine::decide(&call, &allowed), Verdict::Allow, "the rule that names the install lets it run");
    let other = PermissionRules::default().with(RuleKind::Allow, "LSP(install:gopls)");
    assert!(decide("LSP", "hover main.rs", &effects, PermissionMode::Bypass, &other).asks(), "another server's rule");
    let denied = PermissionRules::default().with(RuleKind::Deny, "LSP(install:rust-analyzer)");
    assert!(decide("LSP", "hover main.rs", &effects, PermissionMode::Bypass, &denied).denies());
}

#[test]
fn reading_by_an_absolute_path_inside_the_project_runs_without_asking_while_accepting_edits() {
    let (none, edits) = (PermissionRules::default(), PermissionMode::AcceptEdits);
    let run = |command: &str| decide("Bash", command, &[Effect::exec(command, false)], edits, &none);
    for command in [
        "file /work/rocket/app.exe",
        "ls -la /work/rocket/",
        "grep -n main /work/rocket/src/main.rs | head -5",
        "cat /data/results/run.log",
    ] {
        assert_eq!(run(command), Verdict::Allow, "{command}");
    }
    for command in ["file /etc/hosts", "ls -la /work/rocketeer", "cat /work/rocket/../other/x", "/work/rocket/bin/ls"] {
        assert!(run(command).asks(), "{command}");
    }
}

#[test]
fn a_read_rule_holds_for_a_command_that_reads_inside_the_project() {
    let bounds = bounds();
    let fresh = Remembered::default();
    let denied = PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)");
    let judged = |command: &str, mode: PermissionMode, rules: &PermissionRules| {
        let effects = [Effect::exec(command, false)];
        PolicyEngine::decide(
            &Call { tool: "Bash", subject: command, effects: &effects },
            &context(mode, rules, &fresh, &bounds),
        )
    };
    let reaching = [
        "cat secrets/key",
        "cat ./secrets/key",
        "head -n 5 secrets/a",
        "grep -r token secrets",
        "grep -rn token .",
        "rg -n . secrets",
        "rg token",
        "find . -name '*.pem'",
        "ls -R secrets",
        "wc -l src/main.rs secrets/a",
        "cat /work/rocket/secrets/key",
        "git diff -- secrets/key",
    ];
    for mode in PermissionMode::ALL {
        for command in reaching {
            assert!(
                judged(command, mode, &denied).denies(),
                "{command} in {mode:?}: {:?}",
                judged(command, mode, &denied)
            );
        }
        for command in ["cat src/main.rs", "grep -rn token src", "rg -n fn src", "ls", "ls secrets_notes", "git status"]
        {
            assert_eq!(judged(command, mode, &denied), Verdict::Allow, "{command} in {mode:?}");
        }
    }
    let asked = PermissionRules::default().with(RuleKind::Ask, "Read(secrets/**)");
    assert!(judged("cat secrets/key", PermissionMode::AcceptEdits, &asked).asks());
    let allowed =
        PermissionRules::default().with(RuleKind::Allow, "Bash(cat:*)").with(RuleKind::Ask, "Read(secrets/**)");
    assert!(
        judged("cat secrets/key", PermissionMode::AcceptEdits, &allowed).asks(),
        "a read rule's question outranks it"
    );
    let none = PermissionRules::default();
    assert_eq!(judged("cat secrets/key", PermissionMode::Plan, &none), Verdict::Allow, "no rule, nothing changes");
}

#[test]
fn reading_outside_the_project_by_a_command_asks_about_the_folder_like_reading_a_file() {
    let (none, edits) = (PermissionRules::default(), PermissionMode::AcceptEdits);
    let bounds = bounds();
    let judged = |command: &str, rules: &PermissionRules, remembered: &Remembered| {
        let effects = [Effect::exec(command, false)];
        PolicyEngine::decide(
            &Call { tool: "Bash", subject: command, effects: &effects },
            &context(edits, rules, remembered, &bounds),
        )
    };
    let fresh = Remembered::default();
    assert_eq!(
        judged("ls -la /opt/postman/storage 2>/dev/null | head -20", &none, &fresh),
        Verdict::Ask {
            reason: Reason::Outside("/opt/postman/storage".into()),
            rememberable: true,
            rule: Some("Read(//opt/postman/**)".into()),
        }
    );
    assert!(matches!(
        judged("ls -la ~/Library/Postman", &none, &fresh),
        Verdict::Ask { reason: Reason::Outside(path), .. } if path == std::path::Path::new("/home/ana/Library/Postman")
    ));
    let mut answered = Remembered::default();
    answered.allow_rule("Read(//opt/postman/**)");
    assert_eq!(judged("cat /opt/postman/storage/settings.json", &none, &answered), Verdict::Allow);
    assert!(judged("cat /etc/hosts", &none, &answered).asks(), "another folder still asks");
    let refused = PermissionRules::default().with(RuleKind::Deny, "Read(//etc/**)");
    assert!(judged("grep -n root /etc/passwd", &refused, &fresh).denies(), "a read rule's denial holds");
    for acting in ["rm /opt/postman/x", "cat /opt/../etc/hosts", "cat \"$HOME\"/x"] {
        assert!(
            matches!(judged(acting, &none, &fresh), Verdict::Ask { reason: Reason::Mode, .. }),
            "{acting} is no outside read"
        );
    }
}

struct Linked;

impl Surroundings for Linked {
    fn real_path(&self, path: &std::path::Path) -> PathBuf {
        match path.strip_prefix(CWD) {
            Ok(rest) if rest.starts_with("link") => PathBuf::from("/home/ana/.ssh/id_ed25519"),
            Ok(rest) if rest.starts_with("docs") => PathBuf::from(CWD).join("documents").join(rest),
            _ => path.to_path_buf(),
        }
    }
}

#[test]
fn a_command_reading_through_a_link_is_judged_by_where_the_link_leads() {
    let (none, fresh, bounds) = (PermissionRules::default(), Remembered::default(), bounds());
    let judged = |command: &str, mode: PermissionMode, rules: &PermissionRules| {
        let effects = [Effect::exec(command, false)];
        let cx = PolicyContext { surroundings: &Linked, ..context(mode, rules, &fresh, &bounds) };
        PolicyEngine::decide(&Call { tool: "Bash", subject: command, effects: &effects }, &cx)
    };
    for mode in [PermissionMode::Plan, PermissionMode::Ask, PermissionMode::AcceptEdits] {
        for command in ["cat ./link", "head -n 3 link"] {
            assert!(
                matches!(judged(command, mode, &none), Verdict::Ask { reason: Reason::Outside(_), .. }),
                "{mode:?} {command}: {:?}",
                judged(command, mode, &none)
            );
        }
        assert!(!matches!(judged("wc -l < link", mode, &none), Verdict::Allow), "{mode:?}");
        assert_eq!(judged("cat docs/a.md", mode, &none), Verdict::Allow, "a link that stays inside reads freely");
    }
    let refused = PermissionRules::default().with(RuleKind::Deny, "Read(~/.ssh/**)");
    assert!(judged("cat ./link", PermissionMode::Bypass, &refused).denies(), "a read rule judges where it leads");
}
