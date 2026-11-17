use serde::{Deserialize, Serialize};

use crate::permission_rules::RuleKind;
use crate::{Decision, EngineEvent};

const ENOUGH: usize = 2;

pub const WAITING: &str = "learning";

pub fn offered(proposal: &Proposal) -> String {
    let what = match proposal.kind {
        Lesson::Memory => format!("remember \"{}\"", proposal.text),
        Lesson::Skill => format!("write the skill `{}`: {}", proposal.name, proposal.text),
        Lesson::Rule => {
            format!("keep the {} rule {}", if proposal.name == "deny" { "deny" } else { "allow" }, proposal.text)
        }
        Lesson::Instruction => format!("add to {}: {}", brand::INSTRUCTIONS, proposal.text),
    };
    format!("Lesson from this run: {what}\nWhy: {}\n/learn keep saves it, /learn skip drops it.", proposal.why)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Signals(Vec<String>);

impl Signals {
    pub fn observe(&mut self, event: &EngineEvent) {
        let noted = match event {
            EngineEvent::Answered { decision: Decision::DenyWith(note), .. } => {
                Some(format!("The user refused a tool call and said what to do instead: {note}"))
            }
            EngineEvent::Steered(text) => Some(format!("The user redirected the agent mid-task: {text}")),
            EngineEvent::Repeated { tool } => Some(format!("The agent kept repeating the same {tool} call")),
            EngineEvent::Verified(verdict) if !verdict.passed => {
                Some("The verification judge found the work did not meet the criteria".into())
            }
            _ => None,
        };
        self.0.extend(noted);
    }

    pub fn worth_learning(&self) -> bool {
        self.0.len() >= ENOUGH
    }

    pub fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.0)
    }

    pub fn lesson_due(&mut self) -> Option<String> {
        self.worth_learning().then(|| brief(&self.take()))
    }
}

pub fn brief(signals: &[String]) -> String {
    let lines: Vec<String> = signals.iter().map(|signal| format!("- {signal}")).collect();
    format!("What happened in this conversation that the agent could learn from:\n{}", lines.join("\n"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lesson {
    Memory,
    Skill,
    Rule,
    Instruction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub kind: Lesson,
    pub name: String,
    pub text: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keeping {
    Remember(String),
    Rule { kind: RuleKind, rule: String },
    Instruction(String),
    Ask(String),
}

const LEARNED: &str = "## Learned";

pub fn with_learned(instructions: &str, line: &str) -> Result<String, String> {
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        return Err("The instruction is empty.".into());
    }
    if crate::redact::redact(&line, &crate::redact::Known::default()) != line {
        return Err("The instruction holds what looks like a secret, so it was not written.".into());
    }
    let bullet = format!("- {line}");
    if instructions.lines().any(|existing| existing.trim() == bullet) {
        return Ok(instructions.to_owned());
    }
    let mut text = instructions.trim_end().to_owned();
    if !text.lines().any(|existing| existing.trim() == LEARNED) {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(LEARNED);
    }
    Ok(format!("{text}\n{bullet}\n"))
}

pub fn keeping(proposal: &Proposal) -> Keeping {
    match proposal.kind {
        Lesson::Memory => Keeping::Remember(proposal.text.clone()),
        Lesson::Rule => {
            let kind = if proposal.name == "deny" { RuleKind::Deny } else { RuleKind::Allow };
            Keeping::Rule { kind, rule: proposal.text.clone() }
        }
        Lesson::Instruction => Keeping::Instruction(proposal.text.clone()),
        Lesson::Skill => Keeping::Ask(format!(
            "Create or update the skill `{}` for this: {}\n\nLoad the skill-creator skill and follow it.",
            proposal.name, proposal.text
        )),
    }
}

pub fn parse(reply: &str) -> Option<Proposal> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    let proposal: Proposal = serde_json::from_str(reply.get(start..=end)?).ok()?;
    (!proposal.text.trim().is_empty()).then_some(proposal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AskId;

    #[test]
    fn corrections_from_the_user_are_signals_and_two_are_worth_a_lesson() {
        let mut signals = Signals::default();
        signals.observe(&EngineEvent::Answered { ask: AskId("a".into()), decision: Decision::Deny });
        signals.observe(&EngineEvent::TextDelta("hi".into()));
        assert!(!signals.worth_learning());

        signals.observe(&EngineEvent::Answered {
            ask: AskId("b".into()),
            decision: Decision::DenyWith("use pnpm".into()),
        });
        signals.observe(&EngineEvent::Steered("no, keep the old API".into()));

        assert!(signals.worth_learning());
        let taken = signals.take();
        assert!(taken[0].contains("use pnpm") && taken[1].contains("keep the old API"));
        assert!(!signals.worth_learning(), "taking them starts over");
        assert!(brief(&taken).contains("- The user redirected the agent mid-task: no, keep the old API"));
    }

    #[test]
    fn a_lesson_is_due_once_two_corrections_were_seen_and_asking_starts_over() {
        let mut signals = Signals::default();
        signals.observe(&EngineEvent::Steered("no, keep the old API".into()));
        assert_eq!(signals.lesson_due(), None, "one correction is not enough");
        assert!(!signals.take().is_empty(), "and what was seen stays until it is");

        signals.observe(&EngineEvent::Steered("no, keep the old API".into()));
        signals.observe(&EngineEvent::Repeated { tool: "Bash".into() });
        let brief = signals.lesson_due().expect("due");
        assert!(brief.starts_with("What happened in this conversation that the agent could learn from:"), "{brief}");
        assert!(brief.contains("- The agent kept repeating the same Bash call"), "{brief}");
        assert_eq!(signals.lesson_due(), None, "asked for once");
    }

    #[test]
    fn a_proposal_is_read_from_json_and_an_empty_one_is_none() {
        let reply = r#"Sure: {"kind": "memory", "name": "package manager", "text": "This project uses pnpm, never npm.", "why": "the user said so twice"}"#;
        let proposal = parse(reply).expect("proposal");
        assert_eq!(proposal.kind, Lesson::Memory);
        assert_eq!(proposal.text, "This project uses pnpm, never npm.");
        assert_eq!(parse(r#"{"kind": "rule", "name": "", "text": " ", "why": ""}"#), None);
        assert_eq!(parse("none"), None);
    }

    #[test]
    fn keeping_a_lesson_remembers_it_keeps_a_rule_or_asks_for_the_skill() {
        let lesson =
            |kind, name: &str, text: &str| Proposal { kind, name: name.into(), text: text.into(), why: String::new() };
        assert_eq!(keeping(&lesson(Lesson::Memory, "pm", "Use pnpm.")), Keeping::Remember("Use pnpm.".into()));
        assert_eq!(
            keeping(&lesson(Lesson::Rule, "deny", "Bash(git push:*)")),
            Keeping::Rule { kind: RuleKind::Deny, rule: "Bash(git push:*)".into() }
        );
        assert_eq!(
            keeping(&lesson(Lesson::Rule, "whatever", "Bash(ls)")),
            Keeping::Rule { kind: RuleKind::Allow, rule: "Bash(ls)".into() }
        );
        let Keeping::Ask(prompt) = keeping(&lesson(Lesson::Skill, "release", "Tag then publish.")) else {
            panic!("a skill is asked for")
        };
        assert!(prompt.starts_with("Create or update the skill `release` for this: Tag then publish."));
    }

    #[test]
    fn an_instruction_is_added_once_under_a_learned_section() {
        let lesson = Proposal {
            kind: Lesson::Instruction,
            name: String::new(),
            text: "Run cargo nextest, not cargo test.".into(),
            why: "the user said so".into(),
        };
        assert_eq!(keeping(&lesson), Keeping::Instruction("Run cargo nextest, not cargo test.".into()));
        assert!(offered(&lesson).contains(&format!("add to {}: Run cargo nextest", brand::INSTRUCTIONS)));

        let first = with_learned("# Project\nUse Rust.\n", "Run cargo nextest.").expect("added");
        assert_eq!(first, "# Project\nUse Rust.\n\n## Learned\n- Run cargo nextest.\n");
        let second = with_learned(&first, "Keep commits small.").expect("added");
        assert_eq!(second, "# Project\nUse Rust.\n\n## Learned\n- Run cargo nextest.\n- Keep commits small.\n");
        assert_eq!(with_learned(&second, "Run cargo nextest.").as_deref(), Ok(second.as_str()), "once");
        assert_eq!(with_learned("", "Be brief.").as_deref(), Ok("## Learned\n- Be brief.\n"));
        assert!(with_learned("", "Use token=abc123 for the API.").is_err(), "no secrets in instructions");
        assert!(with_learned("", "  ").is_err());
    }

    #[test]
    fn a_lesson_says_what_would_be_kept_and_why() {
        let proposal = Proposal {
            kind: Lesson::Rule,
            name: "deny".into(),
            text: "Bash(git push:*)".into(),
            why: "you refused every push".into(),
        };
        let text = offered(&proposal);
        assert!(text.starts_with("Lesson from this run: keep the deny rule Bash(git push:*)"), "{text}");
        assert!(text.contains("Why: you refused every push") && text.contains("/learn keep"));
    }
}
