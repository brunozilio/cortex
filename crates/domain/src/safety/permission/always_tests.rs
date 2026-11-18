use std::path::PathBuf;

use super::*;
use crate::PermissionMode;
use crate::permission_rules::{PermissionRules, RuleKind};

const CWD: &str = "/work/rocket";

struct Flagged;

impl Surroundings for Flagged {
    fn package_concern(&self, _call: &Call<'_>) -> Option<String> {
        Some("left-pad was published yesterday".to_owned())
    }
}

struct Asked {
    tool: &'static str,
    subject: &'static str,
    effects: Vec<Effect>,
    mode: PermissionMode,
    rules: PermissionRules,
    sandboxed: bool,
    surroundings: &'static dyn Surroundings,
}

impl Asked {
    fn new(tool: &'static str, subject: &'static str, effects: Vec<Effect>) -> Self {
        Self {
            tool,
            subject,
            effects,
            mode: PermissionMode::Ask,
            rules: PermissionRules::default(),
            sandboxed: false,
            surroundings: &NothingAround,
        }
    }

    fn judge(&self, remembered: &Remembered) -> Judgement {
        let bounds = Bounds::of(vec![PathBuf::from(CWD)], Vec::new());
        let cx = PolicyContext {
            mode: self.mode,
            ceiling: None,
            rules: &self.rules,
            remembered,
            bounds: &bounds,
            cwd: CWD,
            home: Some("/home/ana"),
            sandboxed: self.sandboxed,
            sandbox_missing: None,
            no_prompts: false,
            surroundings: self.surroundings,
        };
        PolicyEngine::judge(&Call { tool: self.tool, subject: self.subject, effects: &self.effects }, &cx)
    }

    fn answered_always(&self) -> (Reason, bool, Verdict) {
        let first = self.judge(&Remembered::default());
        let Verdict::Ask { reason, rememberable, rule } = first.verdict.clone() else {
            panic!("{} {} was not asked: {:?}", self.tool, self.subject, first.verdict);
        };
        let mut remembered = Remembered::default();
        match &rule {
            Some(rule) => remembered.allow_rule(rule),
            None => remembered.remember(self.tool, first.standing, &first.subject),
        }
        (reason, rememberable, self.judge(&remembered).verdict)
    }
}

fn place(path: &str) -> Place {
    Place::at(PathBuf::from(path))
}

static FLAGGED: Flagged = Flagged;

fn every_kind_of_ask() -> Vec<Asked> {
    let build = Asked::new("Bash", "cargo build", vec![Effect::exec("cargo build", false)]);
    let outside = Asked::new("Read", "todo", vec![Effect::ReadPath(place("/home/ana/notes/todo.md"))]);
    let git = Asked {
        mode: PermissionMode::AcceptEdits,
        ..Asked::new("Edit", ".git/config", vec![Effect::WritePath(place("/work/rocket/.git/config"))])
    };
    let install = Asked::new(
        "LSP",
        "hover main.rs",
        vec![
            Effect::ReadPath(place("/work/rocket/main.rs")),
            Effect::Install { server: "rust-analyzer".into(), what: "download https://example.invalid/ra.gz".into() },
        ],
    );
    let leaving = Asked {
        mode: PermissionMode::Bypass,
        sandboxed: true,
        ..Asked::new("Bash", "cargo install x", vec![Effect::exec("cargo install x", true)])
    };
    let package = Asked {
        mode: PermissionMode::Bypass,
        surroundings: &FLAGGED,
        ..Asked::new("Bash", "npm install left-pad", vec![Effect::exec("npm install left-pad", false)])
    };
    let by_rule = Asked {
        mode: PermissionMode::AcceptEdits,
        rules: PermissionRules::default().with(RuleKind::Ask, "Bash(make:*)"),
        ..Asked::new("Bash", "make", vec![Effect::exec("make", false)])
    };
    vec![build, outside, git, install, leaving, package, by_rule]
}

#[test]
fn don_t_ask_again_is_offered_only_where_the_answer_stops_the_next_question() {
    let mut kept = Vec::new();
    for asked in every_kind_of_ask() {
        let (reason, rememberable, next) = asked.answered_always();
        assert_eq!(rememberable, !next.asks(), "{reason:?}: offered {rememberable}, next time {next:?}");
        kept.push((reason, rememberable));
    }
    let offered = |wanted: fn(&Reason) -> bool| kept.iter().find(|(reason, _)| wanted(reason)).map(|(_, kept)| *kept);
    assert_eq!(offered(|reason| matches!(reason, Reason::Mode)), Some(true));
    assert_eq!(offered(|reason| matches!(reason, Reason::Install { .. })), Some(true));
    assert_eq!(offered(|reason| matches!(reason, Reason::LeavesTheSandbox)), Some(false));
    assert_eq!(offered(|reason| matches!(reason, Reason::Package(_))), Some(false));
    assert_eq!(offered(|reason| matches!(reason, Reason::Rule(_))), Some(false));
}

#[test]
fn a_question_a_hook_raises_comes_back_however_it_is_answered() {
    assert!(!Reason::Hook.rememberable());
}
