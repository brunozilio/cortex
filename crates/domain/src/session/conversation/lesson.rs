use super::{Entry, Transcript};
use crate::learning::Proposal;

impl Transcript {
    pub fn offer(&mut self, proposal: Proposal) {
        self.push(Entry::Lesson { proposal, kept: None });
    }

    pub fn lesson(&self) -> Option<&Proposal> {
        self.entries.iter().rev().find_map(|entry| match entry {
            Entry::Lesson { proposal, kept: None } => Some(proposal),
            _ => None,
        })
    }

    pub fn decide(&mut self, keep: bool) -> Option<Proposal> {
        let at = self.entries.iter().rposition(|entry| matches!(entry, Entry::Lesson { kept: None, .. }))?;
        let Entry::Lesson { proposal, kept } = &mut self.entries[at] else { return None };
        *kept = Some(keep);
        Some(proposal.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::Lesson;

    fn lesson(text: &str) -> Proposal {
        Proposal { kind: Lesson::Memory, name: String::new(), text: text.into(), why: "said twice".into() }
    }

    #[test]
    fn a_lesson_offered_is_decided_once_and_the_newest_first() {
        let mut transcript = Transcript::default();
        transcript.offer(lesson("old"));
        transcript.offer(lesson("new"));

        assert_eq!(transcript.lesson(), Some(&lesson("new")), "looking decides nothing");
        assert_eq!(transcript.decide(true), Some(lesson("new")));
        assert_eq!(transcript.lesson(), Some(&lesson("old")));
        assert_eq!(transcript.entries[1], Entry::Lesson { proposal: lesson("new"), kept: Some(true) });
        assert_eq!(transcript.decide(false), Some(lesson("old")));
        assert_eq!(transcript.decide(true), None, "nothing is left to decide");
    }
}
