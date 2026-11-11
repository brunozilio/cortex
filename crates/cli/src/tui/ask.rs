//! What the agent is asking: permission for a tool, or questions with
//! choices. Keys pick; a finished pick is a [`Decision`].

use sdk::{AgentAsk, AskKind, Decision, PermissionMode, Question};

/// What a plan can be answered with, in order: the mode to carry it out in,
/// or `None` to keep planning.
const PLAN_CHOICES: [(&str, Option<PermissionMode>); 3] = [
    ("Yes, and auto-accept edits", Some(PermissionMode::AcceptEdits)),
    ("Yes, and manually approve edits", Some(PermissionMode::Ask)),
    ("No, keep planning", None),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub ask: AgentAsk,
    /// The question being answered.
    pub question: usize,
    /// The highlighted choice.
    pub cursor: usize,
    /// The choices picked for each question so far.
    pub picked: Vec<Vec<usize>>,
    /// What the user is typing for the agent after "No" (`Some` once
    /// picked): sent with the refusal.
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Char(char),
    Up,
    Down,
    Enter,
    Esc,
    Backspace,
}

impl Pending {
    pub fn new(ask: AgentAsk) -> Self {
        Self { ask, question: 0, cursor: 0, picked: Vec::new(), note: None }
    }

    pub fn questions(&self) -> &[Question] {
        match &self.ask.kind {
            AskKind::Questions(questions) => questions,
            AskKind::Permission { .. } | AskKind::Plan { .. } => &[],
        }
    }

    /// The choices of the current question, Yes / Always / No, or how to
    /// carry out a plan.
    pub fn choices(&self) -> Vec<String> {
        match &self.ask.kind {
            AskKind::Permission { rememberable, rule, .. } => {
                let mut choices = vec!["Yes".to_owned()];
                if *rememberable {
                    choices.push(match rule {
                        Some(rule) => format!("Yes, and don't ask again for {rule} in this project"),
                        None => "Yes, and don't ask again this session".to_owned(),
                    });
                }
                choices.push("No, and tell the agent what to do instead".to_owned());
                choices
            }
            AskKind::Questions(questions) => questions
                .get(self.question)
                .map(|q| q.options.iter().map(|o| o.label.clone()).collect())
                .unwrap_or_default(),
            AskKind::Plan { .. } => PLAN_CHOICES.iter().map(|(label, _)| (*label).to_owned()).collect(),
        }
    }

    fn multi(&self) -> bool {
        self.questions().get(self.question).is_some_and(|q| q.multi)
    }

    /// Handles a key; `Some` once the ask is decided.
    pub fn key(&mut self, key: Key) -> Option<Decision> {
        if let Some(note) = &mut self.note {
            return match key {
                Key::Char(c) => {
                    note.push(c);
                    None
                }
                Key::Backspace => {
                    note.pop();
                    None
                }
                Key::Enter if note.trim().is_empty() => Some(Decision::Deny),
                Key::Enter => Some(Decision::DenyWith(note.trim().to_owned())),
                Key::Esc => Some(Decision::Deny),
                Key::Up | Key::Down => None,
            };
        }
        let count = self.choices().len();
        match key {
            Key::Esc => return Some(Decision::Deny),
            Key::Backspace => {}
            Key::Up => self.cursor = self.cursor.checked_sub(1).unwrap_or(count.saturating_sub(1)),
            Key::Down => self.cursor = if self.cursor + 1 >= count { 0 } else { self.cursor + 1 },
            Key::Char(c) => match (&self.ask.kind, c) {
                (AskKind::Permission { .. }, 'y') => return Some(Decision::Allow),
                (AskKind::Permission { rememberable: true, .. }, 'a') => return Some(Decision::AllowAlways),
                (AskKind::Permission { .. }, 'n') => return Some(Decision::Deny),
                (_, ' ') if self.multi() => self.toggle(self.cursor),
                (_, digit) => {
                    let at =
                        digit.to_digit(10).and_then(|n| usize::try_from(n).ok()).filter(|n| (1..=count).contains(n));
                    if let Some(n) = at {
                        self.cursor = n - 1;
                        if self.multi() {
                            self.toggle(n - 1);
                        } else {
                            return self.pick();
                        }
                    }
                }
            },
            Key::Enter => return self.pick(),
        }
        None
    }

    fn toggle(&mut self, choice: usize) {
        while self.picked.len() <= self.question {
            self.picked.push(Vec::new());
        }
        let picked = &mut self.picked[self.question];
        match picked.iter().position(|&c| c == choice) {
            Some(at) => {
                picked.remove(at);
            }
            None => picked.push(choice),
        }
    }

    fn pick(&mut self) -> Option<Decision> {
        if let AskKind::Permission { rememberable, .. } = &self.ask.kind {
            return match (self.cursor, rememberable) {
                (0, _) => Some(Decision::Allow),
                (1, true) => Some(Decision::AllowAlways),
                _ => {
                    self.note = Some(String::new());
                    None
                }
            };
        }
        if let AskKind::Plan { .. } = &self.ask.kind {
            return Some(match PLAN_CHOICES.get(self.cursor) {
                Some((_, Some(mode))) => Decision::Approve(*mode),
                _ => Decision::Deny,
            });
        }
        if !self.multi() {
            while self.picked.len() <= self.question {
                self.picked.push(Vec::new());
            }
            self.picked[self.question] = vec![self.cursor];
        } else if self.picked.get(self.question).is_none_or(Vec::is_empty) {
            self.toggle(self.cursor);
        }
        self.question += 1;
        self.cursor = 0;
        if self.question >= self.questions().len() {
            let mut answers = std::mem::take(&mut self.picked);
            answers.iter_mut().for_each(|picked| picked.sort_unstable());
            return Some(Decision::Answer(answers));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use sdk::{AskId, Choice};

    use super::*;

    fn permission(rememberable: bool) -> Pending {
        Pending::new(AgentAsk {
            id: AskId("a".into()),
            kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable, rule: None },
        })
    }

    fn command(rule: Option<&str>) -> Pending {
        let rule = rule.map(str::to_owned);
        let kind = AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable: true, rule };
        Pending::new(AgentAsk { id: AskId("a".into()), kind })
    }

    #[test]
    fn always_names_the_rule_kept_in_the_project_or_says_it_lasts_the_session() {
        let kept = command(Some("Bash(cargo test:*)")).choices();
        assert_eq!(kept[1], "Yes, and don't ask again for Bash(cargo test:*) in this project");
        assert_eq!(command(None).choices()[1], "Yes, and don't ask again this session");
    }

    #[test]
    fn no_with_a_note_tells_the_agent_what_to_do_instead() {
        let mut ask = command(None);
        assert_eq!(ask.key(Key::Char('3')), None, "the note is typed next");
        assert!(ask.note.is_some());
        for c in "use nextest".chars() {
            assert_eq!(ask.key(Key::Char(c)), None, "letters are the note, not shortcuts");
        }
        ask.key(Key::Backspace);
        assert_eq!(ask.key(Key::Enter), Some(Decision::DenyWith("use nextes".into())));
        let mut empty = command(None);
        empty.key(Key::Char('3'));
        assert_eq!(empty.key(Key::Enter), Some(Decision::Deny), "nothing typed is a plain no");
        assert_eq!(command(None).key(Key::Char('n')), Some(Decision::Deny), "n still says no at once");
    }

    fn question(text: &str, multi: bool) -> Question {
        let options =
            ["one", "two", "three"].map(|label| Choice { label: label.into(), description: String::new() }).to_vec();
        Question { header: text.into(), text: text.into(), options, multi }
    }

    #[test]
    fn a_permission_is_decided_by_letter_or_by_choice() {
        assert_eq!(permission(true).key(Key::Char('y')), Some(Decision::Allow));
        assert_eq!(permission(true).key(Key::Char('a')), Some(Decision::AllowAlways));
        assert_eq!(permission(false).key(Key::Char('a')), None, "nothing to remember");
        assert_eq!(permission(false).key(Key::Esc), Some(Decision::Deny));
        let mut ask = permission(false);
        assert_eq!(ask.choices().len(), 2);
        ask.key(Key::Down);
        assert_eq!(ask.key(Key::Enter), None, "no opens the note");
        assert_eq!(ask.key(Key::Enter), Some(Decision::Deny));
    }

    #[test]
    fn a_plan_is_approved_in_a_mode_or_kept_planning() {
        let plan = || Pending::new(AgentAsk { id: AskId("p".into()), kind: AskKind::Plan { plan: "## Plan".into() } });
        assert_eq!(plan().choices().len(), 3);
        assert_eq!(plan().key(Key::Enter), Some(Decision::Approve(PermissionMode::AcceptEdits)));
        assert_eq!(plan().key(Key::Char('2')), Some(Decision::Approve(PermissionMode::Ask)));
        assert_eq!(plan().key(Key::Char('3')), Some(Decision::Deny));
        assert_eq!(plan().key(Key::Esc), Some(Decision::Deny), "esc keeps planning");
        let mut ask = plan();
        ask.key(Key::Up);
        assert_eq!(ask.key(Key::Enter), Some(Decision::Deny), "up from the first wraps to keep planning");
    }

    #[test]
    fn questions_are_answered_one_after_another() {
        let mut ask = Pending::new(AgentAsk {
            id: AskId("q".into()),
            kind: AskKind::Questions(vec![question("Which?", false), question("Which ones?", true)]),
        });
        assert_eq!(ask.key(Key::Char('2')), None, "the first is answered, the second remains");
        assert_eq!(ask.choices(), ["one", "two", "three"]);
        ask.key(Key::Char('3'));
        ask.key(Key::Char('1'));
        assert_eq!(ask.key(Key::Enter), Some(Decision::Answer(vec![vec![1], vec![0, 2]])));
    }
}
